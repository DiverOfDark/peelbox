use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};

use crate::bootstrap::BootstrapContext;
use crate::llm::{ChatMessage, LLMRequest};
use crate::output::UniversalBuild;
use crate::progress::{NoOpHandler, ProgressEvent, ProgressHandler};
use crate::tools::ToolSystem;

use super::context::PipelineContext;

const SYSTEM_PROMPT: &str = r#"You are an expert build system analyzer. Your task is to analyze a repository and generate UniversalBuild specification(s).

You have access to tools to explore the repository:
- list_files: List files in a directory with optional glob filtering
- read_file: Read file contents (respects size limits)
- search_files: Search for files by name pattern
- get_file_tree: Get tree view of directory structure
- grep_content: Search file contents with regex
- get_best_practices: Get build template for a language/build system
- submit_detection: Submit your final result (single build or array of builds)

CRITICAL RULES:
1. You MUST call EXACTLY ONE tool per response - never multiple tools
2. You MUST respond with ONLY valid JSON - no explanatory text, no markdown, no commentary
3. Valid tool call format: {"name": "tool_name", "arguments": {"param": "value"}}
4. Do NOT add any text before or after the JSON
5. Do NOT wrap JSON in markdown code blocks
6. Do NOT explain your reasoning - only output JSON tool calls

Analysis workflow:
1. Start by exploring the repository structure
2. Identify if this is a single project or monorepo (multiple runnable applications)
3. Read relevant configuration files (package.json, Cargo.toml, pom.xml, etc.)
4. Use get_best_practices to retrieve language-specific templates
5. Call submit_detection with your result

MONOREPO DETECTION:
- For monorepos with multiple runnable applications, submit an ARRAY of UniversalBuild objects
- Each runnable application (web app, API, CLI tool) gets its own UniversalBuild entry
- Shared libraries or packages do NOT get separate UniversalBuild entries
- Single-project repositories should submit a single UniversalBuild object (not wrapped in array)

WORKSPACE ROOT INDICATORS (strong signals for monorepo):
- Cargo.toml with [workspace] section (Rust)
- package.json with "workspaces" field (npm/yarn/pnpm)
- pom.xml with <modules> tag (Maven multi-module)
- settings.gradle or settings.gradle.kts (Gradle multi-project)
- go.work file (Go workspaces)
- lerna.json, nx.json, turbo.json (JavaScript monorepo tools)

When you see these workspace roots, expect multiple sub-projects beneath them.

Example submit_detection for SINGLE PROJECT:
{"name": "submit_detection", "arguments": {"version": "1.0", "metadata": {...}, "build": {...}, "runtime": {...}}}

Example submit_detection for MONOREPO (2 apps):
{"name": "submit_detection", "arguments": [
  {"version": "1.0", "metadata": {"project_name": "web-app", ...}, "build": {...}, "runtime": {...}},
  {"version": "1.0", "metadata": {"project_name": "api", ...}, "build": {...}, "runtime": {...}}
]}

Example valid response:
{"name": "list_files", "arguments": {"path": ".", "max_depth": 2}}

Example INVALID responses:
- "Let me explore the repository first" (text only - FORBIDDEN)
- [{"name": "list_files", ...}, {"name": "read_file", ...}] (multiple tools - FORBIDDEN)
- ```json\n{"name": "list_files", ...}\n``` (markdown - FORBIDDEN)
"#;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Tool system error: {0}")]
    ToolError(#[from] anyhow::Error),

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Max iterations exceeded: {0}")]
    MaxIterationsExceeded(usize),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Detection failed: {0}")]
    DetectionFailed(String),
}

pub struct AnalysisPipeline {
    context: PipelineContext,
}

impl AnalysisPipeline {
    pub fn new(context: PipelineContext) -> Self {
        Self { context }
    }

    pub async fn analyze(
        &self,
        repo_path: PathBuf,
        bootstrap_context: Option<BootstrapContext>,
        progress: Option<Arc<dyn ProgressHandler>>,
    ) -> Result<Vec<UniversalBuild>, PipelineError> {
        let progress = progress.unwrap_or_else(|| Arc::new(NoOpHandler));

        progress.on_progress(&ProgressEvent::Started {
            repo_path: repo_path.display().to_string(),
        });

        info!(
            "Starting analysis pipeline for repository: {}",
            repo_path.display()
        );

        let tool_system = ToolSystem::new(repo_path.clone())
            .context("Failed to create tool system")
            .map_err(PipelineError::ToolError)?;

        let messages = self.build_initial_messages(bootstrap_context.as_ref(), &progress);

        let tools = tool_system.as_tool_definitions();
        debug!("Initialized {} tools for detection", tools.len());

        let start_time = Instant::now();
        let (results, total_iterations) = self
            .detection_loop(messages, tools, tool_system, &progress)
            .await?;

        progress.on_progress(&ProgressEvent::Completed {
            total_iterations,
            total_time: start_time.elapsed(),
        });

        info!(
            "Analysis completed: {} projects detected in {} iterations",
            results.len(),
            total_iterations
        );

        Ok(results)
    }

    fn build_initial_messages(
        &self,
        bootstrap_context: Option<&BootstrapContext>,
        progress: &Arc<dyn ProgressHandler>,
    ) -> Vec<ChatMessage> {
        let user_message = if let Some(context) = bootstrap_context {
            progress.on_progress(&ProgressEvent::BootstrapComplete {
                languages_detected: context.detections.len(),
                scan_time: Duration::from_millis(0),
            });

            info!(
                "Using bootstrap context with {} pre-scanned detections",
                context.detections.len()
            );

            format!(
                "{}\n\nAnalyze the repository. All file paths are relative to the repository root.",
                context.format_for_prompt()
            )
        } else {
            "Analyze the repository. All file paths are relative to the repository root."
                .to_string()
        };

        vec![
            ChatMessage::system(SYSTEM_PROMPT),
            ChatMessage::user(user_message),
        ]
    }

    async fn detection_loop(
        &self,
        mut messages: Vec<ChatMessage>,
        tools: Vec<crate::llm::ToolDefinition>,
        tool_system: ToolSystem,
        progress: &Arc<dyn ProgressHandler>,
    ) -> Result<(Vec<UniversalBuild>, usize), PipelineError> {
        let max_iterations = self.context.config.max_iterations;
        let mut iteration = 0;
        let mut has_read_file = false;
        let mut consecutive_zero_tool_calls = 0;
        const MAX_CONSECUTIVE_ZERO_TOOL_CALLS: usize = 2;

        loop {
            iteration += 1;

            if iteration > max_iterations {
                let error_msg = format!("Exceeded max iterations ({})", max_iterations);
                error!("{}", error_msg);
                progress.on_progress(&ProgressEvent::Failed {
                    error: error_msg.clone(),
                });
                return Err(PipelineError::MaxIterationsExceeded(max_iterations));
            }

            debug!("Iteration {}/{}", iteration, max_iterations);

            progress.on_progress(&ProgressEvent::LlmRequestStarted { iteration });

            let response = self
                .execute_llm_request(&messages, &tools)
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let has_tool_call = response.tool_call.is_some();

            progress.on_progress(&ProgressEvent::LlmResponseReceived {
                iteration,
                has_tool_call,
                response_time: response.response_time,
            });

            debug!("LLM responded with {} tool call", if has_tool_call { "a" } else { "no" });

            if let Some(ref tool_call) = response.tool_call {
                messages.push(ChatMessage::assistant_with_tools(
                    &response.content,
                    vec![tool_call.clone()],
                ));
            } else {
                messages.push(ChatMessage::assistant(&response.content));
            }

            if !has_tool_call {
                consecutive_zero_tool_calls += 1;

                if consecutive_zero_tool_calls >= MAX_CONSECUTIVE_ZERO_TOOL_CALLS {
                    return Err(PipelineError::InvalidResponse(format!(
                        "LLM did not call any tools after {} attempts",
                        consecutive_zero_tool_calls
                    )));
                }

                warn!(
                    "LLM did not call any tools (attempt {}/{}). Sending reminder.",
                    consecutive_zero_tool_calls, MAX_CONSECUTIVE_ZERO_TOOL_CALLS
                );

                messages.push(ChatMessage::user(
                    "You must call a tool now. Do not respond with text. Use one of the available tools to analyze the repository."
                ));

                continue;
            }

            consecutive_zero_tool_calls = 0;

            if let Some(tool_call) = &response.tool_call {
                if let Some(result) = self
                    .process_tool_call(
                        tool_call,
                        &tool_system,
                        &mut messages,
                        &mut has_read_file,
                        iteration,
                        progress,
                    )
                    .await?
                {
                    return Ok(result);
                }
            }
        }
    }

    async fn execute_llm_request(
        &self,
        messages: &[ChatMessage],
        tools: &[crate::llm::ToolDefinition],
    ) -> Result<crate::llm::LLMResponse> {
        let request = LLMRequest::new(messages.to_vec())
            .with_tools(tools.to_vec())
            .with_temperature(0.3)
            .with_stop_sequences(vec![
                "</thinking>".to_string(),
                "In summary:".to_string(),
                "To reiterate:".to_string(),
                "Let me repeat:".to_string(),
            ]);

        self.context
            .llm_client
            .chat(request)
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }

    async fn process_tool_call(
        &self,
        tool_call: &crate::llm::ToolCall,
        tool_system: &ToolSystem,
        messages: &mut Vec<ChatMessage>,
        has_read_file: &mut bool,
        iteration: usize,
        progress: &Arc<dyn ProgressHandler>,
    ) -> Result<Option<(Vec<UniversalBuild>, usize)>, PipelineError> {
            debug!(
                "Executing tool: {} with call_id: {}",
                tool_call.name, tool_call.call_id
            );

            if tool_call.name == "submit_detection" {
                if !*has_read_file {
                    warn!("LLM submitting detection without reading any files");
                }

                let result = self
                    .handle_submit_detection(&tool_call.arguments, progress)
                    .await?;
                return Ok(Some((result, iteration)));
            }

            if tool_call.name == "read_file" {
                *has_read_file = true;
            }

            let start_time = Instant::now();
            progress.on_progress(&ProgressEvent::ToolExecutionStarted {
                tool_name: tool_call.name.clone(),
                iteration,
            });

            // Execute tool and catch errors to allow LLM self-correction
            let result = match tool_system
                .execute(&tool_call.name, tool_call.arguments.clone())
                .await
            {
                Ok(output) => {
                    progress.on_progress(&ProgressEvent::ToolExecutionComplete {
                        tool_name: tool_call.name.clone(),
                        iteration,
                        execution_time: start_time.elapsed(),
                        success: true,
                    });
                    output
                }
                Err(e) => {
                    warn!("Tool execution failed, returning error to LLM: {}", e);
                    progress.on_progress(&ProgressEvent::ToolExecutionComplete {
                        tool_name: tool_call.name.clone(),
                        iteration,
                        execution_time: start_time.elapsed(),
                        success: false,
                    });
                    serde_json::json!({ "error": e.to_string() })
                }
            };

            messages.push(ChatMessage::tool_response(&tool_call.call_id, result));

        Ok(None)
    }

    async fn handle_submit_detection(
        &self,
        arguments: &serde_json::Value,
        progress: &Arc<dyn ProgressHandler>,
    ) -> Result<Vec<UniversalBuild>, PipelineError> {
        progress.on_progress(&ProgressEvent::ValidationStarted);

        let builds = if arguments.is_array() {
            let vec: Vec<UniversalBuild> = serde_json::from_value(arguments.clone())
                .context("Failed to parse Vec<UniversalBuild> from submit_detection")
                .map_err(|e| PipelineError::InvalidResponse(e.to_string()))?;

            if vec.is_empty() {
                return Err(PipelineError::InvalidResponse(
                    "LLM returned empty build array".to_string()
                ));
            }

            vec
        } else {
            let single: UniversalBuild = serde_json::from_value(arguments.clone())
                .context("Failed to parse UniversalBuild from submit_detection")
                .map_err(|e| PipelineError::InvalidResponse(e.to_string()))?;
            vec![single]
        };

        for build in &builds {
            self.context
                .validator
                .validate(build)
                .map_err(|e| PipelineError::ValidationError(e.to_string()))?;
        }

        progress.on_progress(&ProgressEvent::ValidationComplete {
            warnings: 0,
            errors: 0,
        });

        Ok(builds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_creation() {
        let (context, _temp_dir) = PipelineContext::with_mocks();
        let _pipeline = AnalysisPipeline::new(context);
    }
}
