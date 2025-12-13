//! Embedded LLM client using Candle for local inference

use super::download::ModelDownloader;
use super::hardware::{ComputeDevice, HardwareCapabilities, HardwareDetector};
use super::models::{EmbeddedModel, ModelSelector};
use crate::ai::error::BackendError;
use crate::llm::client::LLMClient;
use crate::llm::types::{LLMRequest, LLMResponse, MessageRole, ToolCall};
use anyhow::{Context, Result};
use async_trait::async_trait;
use candle_core::{quantized::gguf_file, Device, IndexOp, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2::ModelWeights as QuantizedQwen2;
use std::sync::Arc;
use std::time::Instant;
use tokenizers::Tokenizer;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Embedded LLM client for local inference without external dependencies
pub struct EmbeddedClient {
    model: Arc<Mutex<QuantizedQwen2>>,
    tokenizer: Tokenizer,
    device: Device,
    model_info: &'static EmbeddedModel,
    max_tokens: usize,
}

impl EmbeddedClient {
    /// Create a new embedded client with automatic model selection
    ///
    /// Automatically detects hardware and selects the best fitting model.
    /// Downloads the model if not already cached.
    pub async fn new(interactive: bool) -> Result<Self> {
        let capabilities = HardwareDetector::detect();

        let model_info = ModelSelector::select(&capabilities).ok_or_else(|| {
            anyhow::anyhow!(
                "Insufficient RAM for embedded LLM. Need at least 3GB available (have {:.1}GB)",
                capabilities.available_ram_gb()
            )
        })?;

        Self::with_model(model_info, &capabilities, interactive).await
    }

    /// Create an embedded client with a specific model
    pub async fn with_model(
        model_info: &'static EmbeddedModel,
        capabilities: &HardwareCapabilities,
        interactive: bool,
    ) -> Result<Self> {
        // Download model if needed
        let downloader = ModelDownloader::new()?;
        let model_paths = downloader.download(model_info, interactive)?;

        // Load tokenizer from model's tokenizer repo
        let tokenizer_path = downloader
            .tokenizer_path(model_info)
            .ok_or_else(|| anyhow::anyhow!("Tokenizer not found for model"))?;
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Select compute device
        let device = Self::create_device(capabilities)?;

        info!(
            "Loading {} on {} device...",
            model_info.display_name,
            capabilities.best_device(),
        );

        // Load GGUF model
        // Try selected device first, fallback to CPU if it fails
        let (model, final_device) = match Self::load_gguf_model(&model_paths[0], &device) {
            Ok(model) => (model, device),
            Err(e) if !device.is_cpu() => {
                warn!(
                    "Failed to load GGUF model on {}: {}. Falling back to CPU",
                    capabilities.best_device(),
                    e
                );
                info!("Retrying model load on CPU for better compatibility");
                let cpu_device = Device::Cpu;
                let model = Self::load_gguf_model(&model_paths[0], &cpu_device)
                    .context("Failed to load GGUF model on CPU fallback")?;
                (model, cpu_device)
            }
            Err(e) => return Err(e),
        };

        info!("Model loaded successfully");

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            tokenizer,
            device: final_device,
            model_info,
            max_tokens: 32768,
        })
    }

    /// Create the compute device based on capabilities
    fn create_device(capabilities: &HardwareCapabilities) -> Result<Device> {
        match capabilities.best_device() {
            ComputeDevice::Cuda => {
                #[cfg(feature = "cuda")]
                {
                    Device::new_cuda(0).context("Failed to create CUDA device")
                }
                #[cfg(not(feature = "cuda"))]
                {
                    warn!("CUDA detected but not compiled with cuda feature, falling back to CPU");
                    Ok(Device::Cpu)
                }
            }
            ComputeDevice::Metal => {
                #[cfg(feature = "metal")]
                {
                    Device::new_metal(0).context("Failed to create Metal device")
                }
                #[cfg(not(feature = "metal"))]
                {
                    warn!(
                        "Metal detected but not compiled with metal feature, falling back to CPU"
                    );
                    Ok(Device::Cpu)
                }
            }
            ComputeDevice::Cpu => Ok(Device::Cpu),
        }
    }

    /// Load a quantized GGUF model
    fn load_gguf_model(model_path: &std::path::Path, device: &Device) -> Result<QuantizedQwen2> {
        debug!("Loading GGUF model from: {}", model_path.display());

        let mut file = std::fs::File::open(model_path).context("Failed to open GGUF model file")?;

        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| anyhow::anyhow!("Failed to read GGUF file: {}", e))?;

        debug!("GGUF file loaded, initializing model weights...");

        let model = QuantizedQwen2::from_gguf(content, &mut file, device)
            .context("Failed to load model weights from GGUF")?;

        debug!("Model weights loaded successfully");

        Ok(model)
    }

    /// Generate text completion
    async fn generate(&self, prompt: &str, max_new_tokens: usize) -> Result<String> {
        let start = Instant::now();

        // Tokenize input
        let encoding = self
            .tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let input_ids: Vec<u32> = encoding.get_ids().to_vec();
        let input_len = input_ids.len();

        debug!("Input tokens: {}", input_len);

        // Generate tokens
        let mut model = self.model.lock().await;

        let mut logits_processor = LogitsProcessor::new(42, Some(0.7), Some(0.9));

        let mut generated_tokens: Vec<u32> = Vec::new();

        for i in 0..max_new_tokens {
            // For autoregressive generation with KV cache:
            // - First iteration: pass full prompt, seqlen_offset = 0
            // - Subsequent iterations: pass only last token, seqlen_offset = tokens already cached
            let (current_input, seqlen_offset) = if i == 0 {
                // First iteration: pass full prompt
                (Tensor::new(input_ids.as_slice(), &self.device)?.unsqueeze(0)?, 0)
            } else {
                // Subsequent iterations: pass only the last generated token
                (Tensor::new(&[generated_tokens.last().copied().unwrap()], &self.device)?.unsqueeze(0)?, input_len + i - 1)
            };

            let logits = model.forward(&current_input, seqlen_offset)?;

            // Get logits for last position - shape depends on model type
            // Full model: [batch, seq_len, vocab] -> get last seq position
            // Quantized model: [batch, vocab] -> squeeze only batch
            let logits = if logits.dims().len() == 3 {
                // Full model - extract logits for the last token in the sequence
                let seq_len = logits.dim(1)?;
                logits.i((0, seq_len - 1))? // [vocab_size]
            } else {
                // Quantized model - already [batch, vocab]
                logits.squeeze(0)?
            };

            // Sample next token
            let next_token = logits_processor.sample(&logits)?;

            // Check for EOS
            if next_token == 151645 || next_token == 151643 {
                // Qwen EOS tokens
                break;
            }

            generated_tokens.push(next_token);
        }

        // Decode output
        let output = self
            .tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decoding failed: {}", e))?;

        debug!(
            "Generated {} tokens in {:.2}s",
            generated_tokens.len(),
            start.elapsed().as_secs_f64()
        );

        Ok(output)
    }

    /// Format tool schemas for injection into system prompt
    fn format_tool_schemas(tools: &[crate::llm::ToolDefinition]) -> String {
        if tools.is_empty() {
            return String::new();
        }

        let mut formatted = String::from("\n\nAvailable Tools:\n\n");

        for tool in tools {
            formatted.push_str(&format!("### {}\n", tool.name));
            formatted.push_str(&format!("{}\n\n", tool.description));

            // For complex nested schemas (like submit_detection), include raw JSON schema
            if tool.name == "submit_detection" {
                formatted.push_str("**Full JSON Schema:**\n```json\n");
                formatted.push_str(&serde_json::to_string_pretty(&tool.parameters).unwrap_or_default());
                formatted.push_str("\n```\n\n");
                continue;
            }

            // For simple schemas, parse and format human-readably
            if let Some(props) = tool.parameters.get("properties").and_then(|p| p.as_object()) {
                formatted.push_str("Parameters:\n");

                let required: Vec<String> = tool.parameters
                    .get("required")
                    .and_then(|r| r.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                for (name, schema) in props {
                    let param_type = schema.get("type").and_then(|t| t.as_str()).unwrap_or("any");
                    let description = schema.get("description").and_then(|d| d.as_str()).unwrap_or("");
                    let is_required = required.contains(&name.to_string());

                    formatted.push_str(&format!(
                        "- {} ({}){}: {}\n",
                        name,
                        param_type,
                        if is_required { ", required" } else { ", optional" },
                        description
                    ));
                }
            }

            formatted.push_str("\n");
        }

        formatted.push_str("**Tool Call Format:**\n");
        formatted.push_str("Output JSON exactly like this:\n");
        formatted.push_str("{\"name\": \"tool_name\", \"arguments\": {\"param1\": \"value1\", \"param2\": \"value2\"}}\n\n");

        formatted
    }

    /// Format messages into a prompt string for Qwen
    fn format_prompt(&self, request: &LLMRequest) -> String {
        let mut prompt = String::new();
        let mut first_system_message = true;

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    prompt.push_str("<|im_start|>system\n");
                    prompt.push_str(&msg.content);

                    // Inject tool schemas after first system message
                    if first_system_message && !request.tools.is_empty() {
                        prompt.push_str(&Self::format_tool_schemas(&request.tools));
                        first_system_message = false;
                    }

                    prompt.push_str("<|im_end|>\n");
                }
                MessageRole::User => {
                    prompt.push_str("<|im_start|>user\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("<|im_end|>\n");
                }
                MessageRole::Assistant => {
                    prompt.push_str("<|im_start|>assistant\n");
                    // Include tool calls in the assistant message if present
                    if let Some(tool_calls) = &msg.tool_calls {
                        for tool_call in tool_calls {
                            let tool_call_json = serde_json::json!({
                                "name": tool_call.name,
                                "arguments": tool_call.arguments
                            });
                            prompt.push_str(&serde_json::to_string(&tool_call_json).unwrap_or_default());
                            prompt.push('\n');
                        }
                    } else {
                        prompt.push_str(&msg.content);
                    }
                    prompt.push_str("<|im_end|>\n");
                }
                MessageRole::Tool => {
                    prompt.push_str("<|im_start|>tool\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("<|im_end|>\n");
                }
            }
        }

        // Add assistant start for generation
        prompt.push_str("<|im_start|>assistant\n");

        prompt
    }

    /// Parse tool calls from generated output
    fn parse_tool_calls(&self, output: &str) -> Vec<ToolCall> {
        let mut tool_calls = Vec::new();
        let mut call_id = 0;

        // Try to parse the entire output as a single JSON tool call
        let trimmed = output.trim();
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Single tool call object
            if let (Some(name), Some(args)) = (
                parsed.get("name").and_then(|n| n.as_str()),
                parsed.get("arguments"),
            ) {
                tool_calls.push(ToolCall {
                    call_id: format!("embedded_{}", call_id),
                    name: name.to_string(),
                    arguments: args.clone(),
                });
                return tool_calls;
            }

            // Array of tool calls
            if let Some(array) = parsed.as_array() {
                for item in array {
                    if let (Some(name), Some(args)) = (
                        item.get("name").and_then(|n| n.as_str()),
                        item.get("arguments"),
                    ) {
                        tool_calls.push(ToolCall {
                            call_id: format!("embedded_{}", call_id),
                            name: name.to_string(),
                            arguments: args.clone(),
                        });
                        call_id += 1;
                    }
                }
                return tool_calls;
            }
        }

        // Fallback: try line-by-line parsing for single-line JSON
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('{') && trimmed.contains("\"name\"") {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let (Some(name), Some(args)) = (
                        parsed.get("name").and_then(|n| n.as_str()),
                        parsed.get("arguments"),
                    ) {
                        tool_calls.push(ToolCall {
                            call_id: format!("embedded_{}", call_id),
                            name: name.to_string(),
                            arguments: args.clone(),
                        });
                        call_id += 1;
                    }
                }
            }
        }

        tool_calls
    }
}

#[async_trait]
impl LLMClient for EmbeddedClient {
    async fn chat(&self, request: LLMRequest) -> Result<LLMResponse, BackendError> {
        let start = Instant::now();

        // Format the prompt
        let prompt = self.format_prompt(&request);

        debug!("=== LLM Request ===");
        debug!("Tools included: {}", request.tools.len());
        debug!("Prompt length: {} chars", prompt.len());
        debug!("Full prompt:\n{}", prompt);

        // Calculate max tokens
        let max_tokens = request.max_tokens.unwrap_or(self.max_tokens as u32) as usize;
        debug!("Max tokens: {}", max_tokens);

        // Generate response
        let output = self
            .generate(&prompt, max_tokens)
            .await
            .map_err(|e| BackendError::Other {
                message: format!("Embedded LLM generation failed: {}", e),
            })?;

        debug!("=== LLM Response ===");
        debug!("Response length: {} chars", output.len());
        debug!("Full response:\n{}", output);

        // Parse tool calls if any
        let tool_calls = self.parse_tool_calls(&output);
        debug!("Parsed {} tool calls", tool_calls.len());

        // Clean content (remove tool call JSON if present)
        let content = if tool_calls.is_empty() {
            output
        } else {
            output
                .lines()
                .filter(|line| !line.trim().starts_with('{'))
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(LLMResponse::with_tool_calls(
            content,
            tool_calls,
            start.elapsed(),
        ))
    }

    fn name(&self) -> &str {
        "EmbeddedLLM"
    }

    fn model_info(&self) -> Option<String> {
        Some(format!(
            "{} ({})",
            self.model_info.display_name, self.model_info.params
        ))
    }
}

impl std::fmt::Debug for EmbeddedClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddedClient")
            .field("model", &self.model_info.display_name)
            .field("device", &format!("{:?}", self.device))
            .finish()
    }
}
