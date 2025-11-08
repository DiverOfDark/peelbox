//! GenAI multi-provider LLM client
//!
//! This module provides a unified interface to multiple LLM providers using the
//! `genai` crate. It supports Ollama, Anthropic Claude, OpenAI, Google Gemini,
//! and other providers through a consistent API.
//!
//! # Example
//!
//! ```no_run
//! use aipack::ai::backend::LLMBackend;
//! use aipack::ai::genai_backend::{GenAIBackend, Provider};
//! use aipack::detection::types::RepositoryContext;
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an Ollama client
//! let client = GenAIBackend::new(
//!     Provider::Ollama,
//!     "qwen2.5-coder:7b".to_string(),
//!     None,
//! ).await?;
//!
//! // Detect build system
//! let context = RepositoryContext::minimal(
//!     PathBuf::from("/path/to/repo"),
//!     "repo/\n├── Cargo.toml\n└── src/".to_string(),
//! );
//!
//! let result = client.detect(context).await?;
//! println!("Detected: {}", result.build_system);
//! # Ok(())
//! # }
//! ```

use crate::ai::backend::{BackendError, LLMBackend};
use crate::detection::prompt::PromptBuilder;
use crate::detection::response::parse_ollama_response;
use crate::detection::types::{DetectionResult, RepositoryContext};
use async_trait::async_trait;
use clap::ValueEnum;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use std::time::Duration;
use tracing::{debug, error, info};

/// Supported LLM providers
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Ollama local inference
    Ollama,
    /// OpenAI GPT models
    #[value(name = "openai")]
    OpenAI,
    /// Anthropic Claude
    Claude,
    /// Google Gemini
    Gemini,
    /// xAI Grok
    Grok,
    /// Groq
    Groq,
}

impl Provider {
    /// Returns the provider prefix for genai model strings
    fn prefix(&self) -> &'static str {
        match self {
            Provider::Ollama => "ollama",
            Provider::Claude => "claude",
            Provider::OpenAI => "openai",
            Provider::Gemini => "gemini",
            Provider::Grok => "grok",
            Provider::Groq => "groq",
        }
    }

    /// Returns the provider name for logging
    fn name(&self) -> &'static str {
        match self {
            Provider::Ollama => "Ollama",
            Provider::Claude => "Claude",
            Provider::OpenAI => "OpenAI",
            Provider::Gemini => "Gemini",
            Provider::Grok => "Grok",
            Provider::Groq => "Groq",
        }
    }

    /// Returns the AdapterKind for genai ServiceTarget
    fn adapter_kind(&self) -> AdapterKind {
        match self {
            Provider::Ollama => AdapterKind::Ollama,
            Provider::OpenAI => AdapterKind::OpenAI,
            Provider::Claude => AdapterKind::Anthropic,
            Provider::Gemini => AdapterKind::Gemini,
            Provider::Grok => AdapterKind::Groq, // xAI uses OpenAI-compatible API
            Provider::Groq => AdapterKind::Groq,
        }
    }

    /// Reads custom endpoint from environment variable
    fn custom_endpoint(&self) -> Option<String> {
        match self {
            Provider::Ollama => std::env::var("OLLAMA_HOST").ok(),
            Provider::OpenAI => std::env::var("OPENAI_API_BASE").ok(),
            Provider::Claude => std::env::var("ANTHROPIC_BASE_URL").ok(),
            Provider::Gemini => std::env::var("GOOGLE_API_BASE_URL").ok(),
            Provider::Grok => std::env::var("XAI_BASE_URL").ok(),
            Provider::Groq => std::env::var("GROQ_BASE_URL").ok(),
        }
    }

    /// Returns the environment variable name for API key
    fn api_key_env_var(&self) -> &'static str {
        match self {
            Provider::Ollama => "", // Ollama doesn't require API key
            Provider::OpenAI => "OPENAI_API_KEY",
            Provider::Claude => "ANTHROPIC_API_KEY",
            Provider::Gemini => "GOOGLE_API_KEY",
            Provider::Grok => "XAI_API_KEY",
            Provider::Groq => "GROQ_API_KEY",
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::Ollama => write!(f, "ollama"),
            Provider::OpenAI => write!(f, "openai"),
            Provider::Claude => write!(f, "claude"),
            Provider::Gemini => write!(f, "gemini"),
            Provider::Grok => write!(f, "grok"),
            Provider::Groq => write!(f, "groq"),
        }
    }
}

/// GenAI-based LLM backend supporting multiple providers
///
/// This client uses the `genai` crate to provide a unified interface across
/// multiple LLM providers. It implements the `LLMBackend` trait to provide
/// consistent build system detection capabilities.
///
/// # Thread Safety
///
/// This client is thread-safe and can be shared across threads using `Arc`.
pub struct GenAIBackend {
    /// GenAI client instance
    client: Client,

    /// Full model identifier (e.g., "ollama:qwen2.5-coder:7b")
    model: String,

    /// Provider type
    provider: Provider,

    /// Request timeout
    timeout: Duration,

    /// Maximum tokens for response
    max_tokens: Option<u32>,
}

impl GenAIBackend {
    /// Creates a new GenAI backend with default settings
    ///
    /// # Arguments
    ///
    /// * `provider` - LLM provider to use
    /// * `model` - Model name (without provider prefix)
    ///
    /// # Note
    ///
    /// Custom endpoints are configured via environment variables:
    /// - `OLLAMA_HOST` for Ollama (default: http://localhost:11434)
    /// - `ANTHROPIC_BASE_URL` for Claude
    /// - `OPENAI_API_BASE` for OpenAI
    /// - `GOOGLE_API_BASE_URL` for Gemini
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::ai::genai_backend::{GenAIBackend, Provider};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Ollama with default endpoint
    /// let ollama_client = GenAIBackend::new(
    ///     Provider::Ollama,
    ///     "qwen2.5-coder:7b".to_string(),
    /// ).await?;
    ///
    /// // Claude (requires ANTHROPIC_API_KEY environment variable)
    /// let claude_client = GenAIBackend::new(
    ///     Provider::Claude,
    ///     "claude-sonnet-4-5-20250929".to_string(),
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(
        provider: Provider,
        model: String,
    ) -> Result<Self, BackendError> {
        Self::with_config(provider, model, None, None).await
    }

    /// Creates a new GenAI backend with custom configuration
    ///
    /// # Arguments
    ///
    /// * `provider` - LLM provider to use
    /// * `model` - Model name (without provider prefix)
    /// * `timeout` - Optional request timeout
    /// * `max_tokens` - Optional maximum tokens for response
    ///
    /// # Note
    ///
    /// Custom endpoints are configured via environment variables:
    /// - `OLLAMA_HOST` for Ollama (default: http://localhost:11434)
    /// - `OPENAI_API_BASE` for OpenAI (default: https://api.openai.com/v1)
    /// - `ANTHROPIC_BASE_URL` for Claude
    /// - `GOOGLE_API_BASE_URL` for Gemini
    /// - `XAI_BASE_URL` for Grok
    /// - `GROQ_BASE_URL` for Groq
    ///
    /// Set the appropriate environment variable before calling this function.
    pub async fn with_config(
        provider: Provider,
        model: String,
        timeout: Option<Duration>,
        max_tokens: Option<u32>,
    ) -> Result<Self, BackendError> {
        // Check for custom endpoint
        let custom_endpoint = provider.custom_endpoint();

        // Create genai client with custom resolver if endpoint is specified
        let client = if let Some(endpoint_url) = custom_endpoint {
            debug!(
                "Using custom endpoint for {}: {}",
                provider.name(),
                endpoint_url
            );

            // Create a ServiceTargetResolver for the custom endpoint
            let provider_clone = provider;
            let model_clone = model.clone();
            let endpoint_clone = endpoint_url.clone();

            let resolver = ServiceTargetResolver::from_resolver_fn(
                move |_service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error> {
                    // Create endpoint from the custom URL
                    let endpoint = Endpoint::from_owned(endpoint_clone.clone());

                    // Get authentication from environment variable
                    let api_key_var = provider_clone.api_key_env_var();
                    let auth = if !api_key_var.is_empty() {
                        AuthData::from_env(api_key_var)
                    } else {
                        // For Ollama which doesn't require auth
                        AuthData::from_single("")
                    };

                    // Build model identifier
                    let model_iden = ModelIden::new(provider_clone.adapter_kind(), &model_clone);

                    Ok(ServiceTarget {
                        endpoint,
                        auth,
                        model: model_iden,
                    })
                },
            );

            Client::builder()
                .with_service_target_resolver(resolver)
                .build()
        } else {
            // Use default client (reads standard environment variables)
            Client::default()
        };

        // Build full model string (e.g., "ollama:qwen2.5-coder:7b")
        let full_model = format!("{}:{}", provider.prefix(), model);

        debug!(
            "Creating GenAI backend: provider={}, model={}",
            provider.name(),
            model,
        );

        Ok(Self {
            client,
            model: full_model,
            provider,
            timeout: timeout.unwrap_or(Duration::from_secs(60)),
            max_tokens,
        })
    }

    /// Internal method to call the GenAI API
    async fn generate(&self, prompt: String) -> Result<String, BackendError> {
        // Build chat request
        let chat_req = ChatRequest::new(vec![ChatMessage::user(prompt.clone())]);

        // Build options
        let mut options = ChatOptions::default().with_temperature(0.3);

        if let Some(max_tokens) = self.max_tokens {
            options = options.with_max_tokens(max_tokens);
        }

        debug!(
            "Sending request to {}: prompt_length={}",
            self.provider.name(),
            prompt.len()
        );

        let start = std::time::Instant::now();

        // Execute chat request
        let response = self
            .client
            .exec_chat(&self.model, chat_req, Some(&options))
            .await
            .map_err(|e| {
                error!("{} API error: {}", self.provider.name(), e);
                BackendError::ApiError {
                    message: format!("{} request failed: {}", self.provider.name(), e),
                    status_code: None,
                }
            })?;

        let elapsed = start.elapsed();

        info!(
            "{} generation completed in {:.2}s",
            self.provider.name(),
            elapsed.as_secs_f64()
        );

        // Extract text content
        let content = response
            .first_text()
            .ok_or_else(|| {
                error!(
                    "No text content in {} response",
                    self.provider.name()
                );
                BackendError::InvalidResponse {
                    message: "No text content in response".to_string(),
                    raw_response: None,
                }
            })?
            .to_string();

        debug!(
            "{} response length: {} characters",
            self.provider.name(),
            content.len()
        );

        Ok(content)
    }
}

#[async_trait]
impl LLMBackend for GenAIBackend {
    /// Detects build system and generates commands from repository context
    ///
    /// This method constructs a prompt from the repository context, sends it to
    /// the configured LLM provider via genai, and parses the response.
    async fn detect(&self, context: RepositoryContext) -> Result<DetectionResult, BackendError> {
        info!(
            "Starting detection for repository: {} using {}",
            context.repo_path.display(),
            self.provider.name()
        );

        // Build the prompt
        let prompt = PromptBuilder::build_detection_prompt(&context);
        debug!("Built prompt with {} characters", prompt.len());

        // Call API
        let response_text = self.generate(prompt).await?;
        debug!("Received response with {} characters", response_text.len());

        // Parse the response
        let mut result = parse_ollama_response(&response_text).map_err(|e| {
            error!("Failed to parse {} response: {}", self.provider.name(), e);
            BackendError::ParseError {
                message: e.to_string(),
                context: response_text.chars().take(200).collect(),
            }
        })?;

        // Set detected files from context if not already set
        if result.detected_files.is_empty() {
            result.detected_files = context.detected_files.clone();
        }

        info!(
            "Detection completed: {} ({}) with {:.1}% confidence",
            result.build_system,
            result.language,
            result.confidence * 100.0
        );

        Ok(result)
    }

    fn name(&self) -> &str {
        self.provider.name()
    }

    fn model_info(&self) -> Option<String> {
        Some(self.model.clone())
    }
}

impl std::fmt::Debug for GenAIBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenAIBackend")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("timeout", &self.timeout)
            .field("max_tokens", &self.max_tokens)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_prefix() {
        assert_eq!(Provider::Ollama.prefix(), "ollama");
        assert_eq!(Provider::Claude.prefix(), "claude");
        assert_eq!(Provider::OpenAI.prefix(), "openai");
        assert_eq!(Provider::Gemini.prefix(), "gemini");
    }

    #[tokio::test]
    async fn test_provider_name() {
        assert_eq!(Provider::Ollama.name(), "Ollama");
        assert_eq!(Provider::Claude.name(), "Claude");
        assert_eq!(Provider::OpenAI.name(), "OpenAI");
    }

    #[tokio::test]
    async fn test_backend_creation() {
        let backend = GenAIBackend::new(
            Provider::Ollama,
            "qwen2.5-coder:7b".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(backend.name(), "Ollama");
        assert_eq!(backend.model, "ollama:qwen2.5-coder:7b");
        assert!(backend.model_info().is_some());
    }

    #[tokio::test]
    async fn test_backend_with_custom_config() {
        let backend = GenAIBackend::with_config(
            Provider::Claude,
            "claude-sonnet-4-5".to_string(),
            Some(Duration::from_secs(120)),
            Some(1024),
        )
        .await
        .unwrap();

        assert_eq!(backend.provider, Provider::Claude);
        assert_eq!(backend.timeout, Duration::from_secs(120));
        assert_eq!(backend.max_tokens, Some(1024));
    }

    #[tokio::test]
    async fn test_debug_impl() {
        let backend = GenAIBackend::new(
            Provider::Ollama,
            "qwen2.5-coder:7b".to_string(),
        )
        .await
        .unwrap();

        let debug_str = format!("{:?}", backend);
        assert!(debug_str.contains("GenAIBackend"));
        assert!(debug_str.contains("Ollama"));
    }
}
