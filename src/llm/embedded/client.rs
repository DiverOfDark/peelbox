//! Embedded LLM client using Candle for local inference

use super::download::ModelDownloader;
use super::hardware::{ComputeDevice, HardwareCapabilities, HardwareDetector};
use super::models::{EmbeddedModel, ModelSelector};
use crate::ai::genai_backend::BackendError;
use crate::llm::client::LLMClient;
use crate::llm::types::{LLMRequest, LLMResponse, MessageRole, ToolCall};
use anyhow::{Context, Result};
use async_trait::async_trait;
use candle_core::{quantized::gguf_file, Device, Tensor};
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

        let model_info = ModelSelector::select(&capabilities)
            .ok_or_else(|| anyhow::anyhow!(
                "Insufficient RAM for embedded LLM. Need at least 3GB available (have {:.1}GB)",
                capabilities.available_ram_gb()
            ))?;

        Self::with_model(model_info, &capabilities, interactive).await
    }

    /// Create an embedded client with a specific model
    pub async fn with_model(
        model_info: &'static EmbeddedModel,
        capabilities: &HardwareCapabilities,
        interactive: bool,
    ) -> Result<Self> {
        // Download model and related files if needed
        let downloader = ModelDownloader::new()?;
        let model_paths = downloader.download(model_info, interactive)?;
        let tokenizer_path = downloader
            .tokenizer_path(model_info)
            .ok_or_else(|| anyhow::anyhow!("Tokenizer not found for model"))?;

        // Select compute device
        let device = Self::create_device(capabilities)?;

        info!(
            "Loading {} on {} device...",
            model_info.display_name,
            capabilities.best_device(),
        );

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Load quantized GGUF model
        let model = Self::load_gguf_model(&model_paths[0], &device)?;

        info!("Model loaded successfully");

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            tokenizer,
            device,
            model_info,
            max_tokens: 4096,
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
                    warn!("Metal detected but not compiled with metal feature, falling back to CPU");
                    Ok(Device::Cpu)
                }
            }
            ComputeDevice::Cpu => Ok(Device::Cpu),
        }
    }

    /// Load a quantized GGUF model
    fn load_gguf_model(model_path: &std::path::Path, device: &Device) -> Result<QuantizedQwen2> {
        debug!("Loading GGUF model from: {}", model_path.display());

        let mut file = std::fs::File::open(model_path)
            .context("Failed to open GGUF model file")?;

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
        let encoding = self.tokenizer
            .encode(prompt, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let input_ids: Vec<u32> = encoding.get_ids().to_vec();
        let input_len = input_ids.len();

        debug!("Input tokens: {}", input_len);

        // Create tensor
        let input_tensor = Tensor::new(input_ids.as_slice(), &self.device)?
            .unsqueeze(0)?;

        // Generate tokens
        let mut model = self.model.lock().await;
        let mut logits_processor = LogitsProcessor::new(42, Some(0.7), Some(0.9));

        let mut generated_tokens: Vec<u32> = Vec::new();
        let mut current_input = input_tensor;

        for i in 0..max_new_tokens {
            let logits = model.forward(&current_input, i)?;

            // Get logits for last position
            let logits = logits.squeeze(0)?.squeeze(0)?;

            // Sample next token
            let next_token = logits_processor.sample(&logits)?;

            // Check for EOS
            if next_token == 151645 || next_token == 151643 {
                // Qwen EOS tokens
                break;
            }

            generated_tokens.push(next_token);

            // Prepare next input
            current_input = Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
        }

        // Decode output
        let output = self.tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| anyhow::anyhow!("Decoding failed: {}", e))?;

        debug!(
            "Generated {} tokens in {:.2}s",
            generated_tokens.len(),
            start.elapsed().as_secs_f64()
        );

        Ok(output)
    }

    /// Format messages into a prompt string for Qwen
    fn format_prompt(&self, request: &LLMRequest) -> String {
        let mut prompt = String::new();

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    prompt.push_str("<|im_start|>system\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("<|im_end|>\n");
                }
                MessageRole::User => {
                    prompt.push_str("<|im_start|>user\n");
                    prompt.push_str(&msg.content);
                    prompt.push_str("<|im_end|>\n");
                }
                MessageRole::Assistant => {
                    prompt.push_str("<|im_start|>assistant\n");
                    prompt.push_str(&msg.content);
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
        // Look for JSON tool call patterns in the output
        // Format: {"name": "tool_name", "arguments": {...}}

        let mut tool_calls = Vec::new();
        let mut call_id = 0;

        // Simple regex-free parsing for tool calls
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
        debug!("Prompt length: {} chars", prompt.len());
        if prompt.len() < 2000 {
            debug!("Full prompt:\n{}", prompt);
        } else {
            debug!("Prompt (first 1000 chars):\n{}...", &prompt[..1000]);
        }

        // Calculate max tokens
        let max_tokens = request.max_tokens.unwrap_or(self.max_tokens as u32) as usize;
        debug!("Max tokens: {}", max_tokens);

        // Generate response
        let output = self.generate(&prompt, max_tokens).await.map_err(|e| {
            BackendError::Other {
                message: format!("Embedded LLM generation failed: {}", e),
            }
        })?;

        debug!("=== LLM Response ===");
        debug!("Response length: {} chars", output.len());
        if output.len() < 2000 {
            debug!("Full response:\n{}", output);
        } else {
            debug!("Response (first 1000 chars):\n{}...", &output[..1000]);
        }

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
        Some(format!("{} ({})", self.model_info.display_name, self.model_info.params))
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
