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

const SYSTEM_PROMPT: &str = r#"You are an expert build system analyzer. Your task is to analyze a repository and generate a complete UniversalBuild specification.

You have access to tools to explore the repository:
- list_files: List files in a directory with optional glob filtering
- read_file: Read file contents (respects size limits)
- search_files: Search for files by name pattern
- get_file_tree: Get tree view of directory structure
- grep_content: Search file contents with regex
- get_best_practices: Get build template for a language/build system
- submit_detection: Submit your final UniversalBuild result

CRITICAL RULES:
1. You MUST respond with ONLY valid JSON tool calls - no explanatory text, no markdown, no commentary
2. Every response must be a JSON object or array of JSON objects representing tool calls
3. Valid tool call format: {"name": "tool_name", "arguments": {"param": "value"}}
4. For multiple tool calls: [{"name": "tool1", "arguments": {...}}, {"name": "tool2", "arguments": {...}}]
5. Do NOT add any text before or after the JSON
6. Do NOT wrap JSON in markdown code blocks
7. Do NOT explain your reasoning - only output JSON tool calls

Analysis workflow:
1. Start by exploring the repository structure
2. Identify the primary programming language and build system
3. Read relevant configuration files (package.json, Cargo.toml, pom.xml, etc.)
4. Use get_best_practices to retrieve language-specific templates
5. Call submit_detection with a complete UniversalBuild specification

Example valid response:
{"name": "list_files", "arguments": {"path": ".", "max_depth": 2}}

Example INVALID responses:
- "Let me explore the repository first" (text only - FORBIDDEN)
- "I'll call list_files to check the structure" (text - FORBIDDEN)
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
    ) -> Result<UniversalBuild, PipelineError> {
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
        let (result, total_iterations) = self
            .detection_loop(messages, tools, tool_system, &progress)
            .await?;

        progress.on_progress(&ProgressEvent::Completed {
            total_iterations,
            total_time: start_time.elapsed(),
        });

        info!(
            "Analysis completed: {} ({}) with {:.1}% confidence in {} iterations",
            result.metadata.build_system,
            result.metadata.language,
            result.metadata.confidence * 100.0,
            total_iterations
        );

        Ok(result)
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
    ) -> Result<(UniversalBuild, usize), PipelineError> {
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

            let tool_calls = &response.tool_calls;

            progress.on_progress(&ProgressEvent::LlmResponseReceived {
                iteration,
                tool_calls: tool_calls.len(),
                response_time: response.response_time,
            });

            debug!("LLM responded with {} tool calls", tool_calls.len());

            if tool_calls.is_empty() {
                messages.push(ChatMessage::assistant(&response.content));
            } else {
                // Always add assistant message with tool calls
                // The tool calls JSON will be included when formatting the prompt
                let llm_tool_calls: Vec<crate::llm::ToolCall> = tool_calls
                    .iter()
                    .map(|tc| crate::llm::ToolCall {
                        call_id: tc.call_id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    })
                    .collect();
                messages.push(ChatMessage::assistant_with_tools(
                    &response.content,
                    llm_tool_calls,
                ));
            }

            if tool_calls.is_empty() {
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

            if let Some(result) = self
                .process_tool_calls(
                    tool_calls,
                    &tool_system,
                    &mut messages,
                    &mut has_read_file,
                    iteration,
                    max_iterations,
                    progress,
                )
                .await?
            {
                return Ok(result);
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

    #[allow(clippy::too_many_arguments)]
    async fn process_tool_calls(
        &self,
        tool_calls: &[crate::llm::ToolCall],
        tool_system: &ToolSystem,
        messages: &mut Vec<ChatMessage>,
        has_read_file: &mut bool,
        iteration: usize,
        max_iterations: usize,
        progress: &Arc<dyn ProgressHandler>,
    ) -> Result<Option<(UniversalBuild, usize)>, PipelineError> {
        let has_submit_detection = tool_calls.iter().any(|tc| tc.name == "submit_detection");
        let is_last_iteration = iteration >= max_iterations - 1;
        let is_only_tool_call = tool_calls.len() == 1;

        let should_accept_submit_detection =
            has_submit_detection && (is_only_tool_call || is_last_iteration);

        for tool_call in tool_calls {
            debug!(
                "Executing tool: {} with call_id: {}",
                tool_call.name, tool_call.call_id
            );

            if tool_call.name == "submit_detection" {
                if should_accept_submit_detection {
                    if !*has_read_file {
                        warn!("LLM submitting detection without reading any files");
                    }

                    let result = self
                        .handle_submit_detection(&tool_call.arguments, progress)
                        .await?;
                    return Ok(Some((result, iteration)));
                } else {
                    let warning = "Cannot submit yet. You called submit_detection along with other tools. Please call only submit_detection when ready.";
                    warn!("{}", warning);

                    messages.push(ChatMessage::tool_response(&tool_call.call_id, serde_json::json!({"warning": warning})));
                    continue;
                }
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
        }

        Ok(None)
    }

    async fn handle_submit_detection(
        &self,
        arguments: &serde_json::Value,
        progress: &Arc<dyn ProgressHandler>,
    ) -> Result<UniversalBuild, PipelineError> {
        progress.on_progress(&ProgressEvent::ValidationStarted);

        let universal_build: UniversalBuild = serde_json::from_value(arguments.clone())
            .context("Failed to parse UniversalBuild from submit_detection")
            .map_err(|e| PipelineError::InvalidResponse(e.to_string()))?;

        self.context
            .validator
            .validate(&universal_build)
            .map_err(|e| PipelineError::ValidationError(e.to_string()))?;

        progress.on_progress(&ProgressEvent::ValidationComplete {
            warnings: 0,
            errors: 0,
        });

        Ok(universal_build)
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
