//! GenAI multi-provider LLM client
//!
//! This module provides a unified interface to multiple LLM providers using the
//! `genai` crate. It supports Ollama, Anthropic Claude, OpenAI, Google Gemini,
//! and other providers through a consistent API.
//!
//! # Example
//!
//! ```no_run
//! use aipack::ai::genai_backend::{GenAIBackend, Provider};
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create an Ollama client
//! let client = GenAIBackend::new(
//!     Provider::Ollama,
//!     "qwen2.5-coder:7b".to_string(),
//! ).await?;
//!
//! // Detect build system
//! let result = client.detect(PathBuf::from("/path/to/repo")).await?;
//! println!("Detected: {}", result.build_system);
//! # Ok(())
//! # }
//! ```

use crate::ai::backend::LLMBackend;
use crate::detection::response::parse_ollama_response;
use crate::detection::types::DetectionResult;
use async_trait::async_trait;
use clap::ValueEnum;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ToolResponse};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use reqwest;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// System prompt for the LLM build system detection expert
const SYSTEM_PROMPT: &str = r#"You are an expert build system detection assistant. Your role is to analyze repository structures and accurately identify the build system, language, and configuration.

Available tools:
- list_files: List files in a directory with optional filtering
- read_file: Read the contents of a specific file
- search_files: Search for files by name pattern
- get_file_tree: Get a tree view of the repository structure
- grep_content: Search for text patterns within files
- submit_detection: Submit your final detection result

Process:
1. Start by exploring the repository structure (use get_file_tree or list_files)
2. Identify key configuration files (package.json, Cargo.toml, pom.xml, etc.)
3. Read relevant files to confirm the build system and gather details
4. When confident, call submit_detection with your findings

Be efficient - only request files you need. Focus on identifying:
- Programming language
- Build system (cargo, npm, maven, gradle, make, etc.)
- Build and test commands
- Runtime environment
- Entry points and dependencies

Your detection should be thorough but concise. Aim for high confidence by verifying key indicators."#;

/// Errors that can occur during backend operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendError {
    /// API request failed with the given message
    ApiError {
        message: String,
        status_code: Option<u16>,
    },

    /// Authentication failed or credentials are invalid
    AuthenticationError { message: String },

    /// Request timed out after the specified duration (in seconds)
    TimeoutError { seconds: u64 },

    /// Rate limit exceeded, retry after the specified duration (in seconds)
    RateLimitError { retry_after: Option<u64> },

    /// Invalid or malformed response from the LLM
    InvalidResponse {
        message: String,
        raw_response: Option<String>,
    },

    /// Configuration error (missing API keys, invalid settings, etc.)
    ConfigurationError { message: String },

    /// Network-related error
    NetworkError { message: String },

    /// The LLM response could not be parsed into a DetectionResult
    ParseError { message: String, context: String },

    /// Generic error for other cases
    Other { message: String },
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::ApiError {
                message,
                status_code,
            } => {
                if let Some(code) = status_code {
                    write!(f, "API error ({}): {}", code, message)
                } else {
                    write!(f, "API error: {}", message)
                }
            }
            BackendError::AuthenticationError { message } => {
                write!(f, "Authentication failed: {}", message)
            }
            BackendError::TimeoutError { seconds } => {
                write!(f, "Request timed out after {} seconds", seconds)
            }
            BackendError::RateLimitError { retry_after } => {
                if let Some(seconds) = retry_after {
                    write!(f, "Rate limit exceeded, retry after {} seconds", seconds)
                } else {
                    write!(f, "Rate limit exceeded")
                }
            }
            BackendError::InvalidResponse { message, .. } => {
                write!(f, "Invalid response from LLM: {}", message)
            }
            BackendError::ConfigurationError { message } => {
                write!(f, "Configuration error: {}", message)
            }
            BackendError::NetworkError { message } => {
                write!(f, "Network error: {}", message)
            }
            BackendError::ParseError { message, context } => {
                write!(f, "Parse error: {} (context: {})", message, context)
            }
            BackendError::Other { message } => {
                write!(f, "Error: {}", message)
            }
        }
    }
}

impl std::error::Error for BackendError {}

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

    /// Returns the default base URL for the provider
    fn default_base_url(&self) -> &'static str {
        match self {
            Provider::Ollama => "http://localhost:11434",
            Provider::OpenAI => "https://api.openai.com/v1",
            Provider::Claude => "https://api.anthropic.com",
            Provider::Gemini => "https://generativelanguage.googleapis.com",
            Provider::Grok => "https://api.x.ai/v1",
            Provider::Groq => "https://api.groq.com/openai/v1",
        }
    }

    /// Returns the models list endpoint path for the provider
    fn models_endpoint(&self) -> Option<&'static str> {
        match self {
            Provider::Ollama => Some("/api/tags"),
            Provider::OpenAI => Some("/models"),
            Provider::Grok => Some("/models"),
            Provider::Groq => Some("/models"),
            // Claude and Gemini don't have a standard models list endpoint
            Provider::Claude => None,
            Provider::Gemini => None,
        }
    }

    /// Gets the base URL (custom or default)
    fn base_url(&self) -> String {
        self.custom_endpoint()
            .unwrap_or_else(|| self.default_base_url().to_string())
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

/// OpenAI-compatible models list response
#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

/// Ollama models list response
#[derive(Debug, Deserialize)]
struct OllamaModelsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

/// GenAI-based LLM backend supporting multiple providers
///
/// This client uses the `genai` crate to provide a unified interface across
/// multiple LLM providers. It provides consistent build system detection
/// capabilities across all supported providers.
///
/// # Thread Safety
///
/// This client is thread-safe and can be shared across threads using `Arc`.
pub struct GenAIBackend {
    /// GenAI client instance
    client: Client,

    /// Model name (without provider prefix)
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
                    debug!(
                        "ServiceTargetResolver: creating custom endpoint for {} at {}",
                        provider_clone.name(),
                        endpoint_clone
                    );

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

                    debug!(
                        "ServiceTargetResolver: returning endpoint URL={}, adapter={:?}, model={}",
                        endpoint_clone,
                        provider_clone.adapter_kind(),
                        model_clone
                    );

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

        debug!(
            "Creating GenAI backend: provider={}, model={}",
            provider.name(),
            model,
        );

        let backend = Self {
            client,
            model: model.clone(),
            provider,
            timeout: timeout.unwrap_or(Duration::from_secs(60)),
            max_tokens,
        };

        // Validate that the model exists
        backend.validate_model(&model).await?;

        Ok(backend)
    }

    /// Validates that the requested model is available on the provider
    async fn validate_model(&self, model_name: &str) -> Result<(), BackendError> {
        // Check if provider supports model listing
        let endpoint_path = match self.provider.models_endpoint() {
            Some(path) => path,
            None => {
                debug!(
                    "{} doesn't support model listing, skipping validation",
                    self.provider.name()
                );
                return Ok(());
            }
        };

        let base_url = self.provider.base_url();

        // Use proper URL joining to handle trailing/leading slashes correctly
        let models_url = reqwest::Url::parse(&base_url)
            .and_then(|base| {
                // Remove leading slash from endpoint_path if present for proper joining
                let path = endpoint_path.trim_start_matches('/');
                base.join(path)
            })
            .map_err(|e| BackendError::ConfigurationError {
                message: format!("Failed to construct models URL: {}", e),
            })?
            .to_string();

        debug!(
            "Validating model '{}' against {} endpoint: {}",
            model_name,
            self.provider.name(),
            models_url
        );

        // Build HTTP client with authentication
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| BackendError::ConfigurationError {
                message: format!("Failed to create HTTP client: {}", e),
            })?;

        let mut request_builder = client.get(&models_url);

        // Add authentication header if needed
        let api_key_var = self.provider.api_key_env_var();
        if !api_key_var.is_empty() {
            if let Ok(api_key) = std::env::var(api_key_var) {
                request_builder = request_builder.header("Authorization", format!("Bearer {}", api_key));
            }
        }

        // Make the request
        let response = match request_builder.send().await {
            Ok(resp) => resp,
            Err(e) => {
                warn!(
                    "Failed to fetch models list from {}: {}. Skipping validation.",
                    self.provider.name(),
                    e
                );
                // Don't fail on network errors - just warn and continue
                return Ok(());
            }
        };

        if !response.status().is_success() {
            warn!(
                "Models endpoint returned status {}: {}. Skipping validation.",
                response.status(),
                models_url
            );
            // Don't fail on HTTP errors - just warn and continue
            return Ok(());
        }

        // Parse response based on provider type
        let available_models: Vec<String> = match self.provider {
            Provider::Ollama => {
                let ollama_response: OllamaModelsResponse = response.json().await.map_err(|e| {
                    warn!("Failed to parse Ollama models response: {}", e);
                    BackendError::ConfigurationError {
                        message: format!("Failed to parse models list: {}", e),
                    }
                })?;
                ollama_response.models.into_iter().map(|m| m.name).collect()
            }
            Provider::OpenAI | Provider::Grok | Provider::Groq => {
                let openai_response: OpenAIModelsResponse = response.json().await.map_err(|e| {
                    warn!("Failed to parse OpenAI-compatible models response: {}", e);
                    BackendError::ConfigurationError {
                        message: format!("Failed to parse models list: {}", e),
                    }
                })?;
                openai_response.data.into_iter().map(|m| m.id).collect()
            }
            _ => {
                // Shouldn't reach here due to models_endpoint() check above
                return Ok(());
            }
        };

        debug!(
            "Available models on {}: {:?}",
            self.provider.name(),
            available_models
        );

        // Check if requested model is in the list
        if !available_models.iter().any(|m| m == model_name) {
            error!(
                "Model '{}' not found in {} available models",
                model_name,
                self.provider.name()
            );
            return Err(BackendError::ConfigurationError {
                message: format!(
                    "Model '{}' is not available on {}. Available models: {}",
                    model_name,
                    self.provider.name(),
                    available_models.join(", ")
                ),
            });
        }

        info!(
            "Model '{}' validated successfully on {}",
            model_name,
            self.provider.name()
        );

        Ok(())
    }

}

impl GenAIBackend {
    /// Detects build system using tool-based conversation loop
    ///
    /// This method uses an iterative tool-calling approach where the LLM
    /// can request information about the repository through tools until
    /// it has enough information to submit a final detection result.
    pub async fn detect(&self, repo_path: PathBuf) -> Result<DetectionResult, BackendError> {
        use crate::detection::tools::{ToolExecutor, ToolRegistry};

        info!(
            "Starting tool-based detection for repository: {} using {}",
            repo_path.display(),
            self.provider.name()
        );

        // Create tool executor
        let executor = ToolExecutor::new(repo_path.clone()).map_err(|e| {
            BackendError::Other {
                message: format!("Failed to create tool executor: {}", e),
            }
        })?;

        // Get all tools
        let tools = ToolRegistry::create_all_tools();
        debug!("Initialized {} tools for detection", tools.len());

        // Initial conversation
        let mut messages: Vec<ChatMessage> = vec![
            ChatMessage::system(SYSTEM_PROMPT),
            ChatMessage::user(format!(
                "Analyze the repository at path: {}",
                repo_path.display()
            )),
        ];

        const MAX_ITERATIONS: usize = 10;
        let mut iteration = 0;
        let start = std::time::Instant::now();

        loop {
            iteration += 1;
            if iteration > MAX_ITERATIONS {
                error!("Exceeded max iterations ({})", MAX_ITERATIONS);
                return Err(BackendError::Other {
                    message: format!("Exceeded max iterations ({})", MAX_ITERATIONS),
                });
            }

            debug!("Iteration {}/{}", iteration, MAX_ITERATIONS);

            // Create chat request with tools
            let request = ChatRequest::new(messages.clone()).with_tools(tools.clone());

            // Build options
            let mut options = ChatOptions::default().with_temperature(0.3);
            if let Some(max_tokens) = self.max_tokens {
                options = options.with_max_tokens(max_tokens);
            }

            // Execute LLM request
            let response = self
                .client
                .exec_chat(&self.model, request, Some(&options))
                .await
                .map_err(|e| {
                    error!("{} API error: {}", self.provider.name(), e);
                    BackendError::ApiError {
                        message: format!("{} request failed: {}", self.provider.name(), e),
                        status_code: None,
                    }
                })?;

            debug!(
                "{} responded with {} tool calls",
                self.provider.name(),
                response.tool_calls().len()
            );

            // Add assistant message to conversation (may contain text + tool calls)
            messages.push(ChatMessage::assistant(response.content.clone()));

            // Check for tool calls
            let tool_calls = response.tool_calls();

            if tool_calls.is_empty() {
                warn!("LLM did not call any tools");
                return Err(BackendError::InvalidResponse {
                    message: "LLM did not call any tools".to_string(),
                    raw_response: response.first_text().map(|s| s.to_string()),
                });
            }

            // Execute each tool call
            for tool_call in tool_calls {
                debug!(
                    "Executing tool: {} with call_id: {}",
                    tool_call.fn_name, tool_call.call_id
                );

                // Check if this is submit_detection
                if tool_call.fn_name == "submit_detection" {
                    info!(
                        "Detection submitted after {} iterations in {:.2}s",
                        iteration,
                        start.elapsed().as_secs_f64()
                    );
                    return parse_detection_from_tool_call(&tool_call.fn_arguments);
                }

                // Execute the tool
                let result = executor
                    .execute(&tool_call.fn_name, tool_call.fn_arguments.clone())
                    .await
                    .map_err(|e| {
                        warn!("Tool {} failed: {}", tool_call.fn_name, e);
                        BackendError::Other {
                            message: format!("Tool {} failed: {}", tool_call.fn_name, e),
                        }
                    })?;

                debug!(
                    "Tool {} returned {} bytes",
                    tool_call.fn_name,
                    result.len()
                );

                // Add tool response to conversation
                let tool_response = ToolResponse {
                    call_id: tool_call.call_id.clone(),
                    content: result,
                };
                messages.push(tool_response.into());
            }
        }
    }

    /// Returns the human-readable name of this backend
    pub fn name(&self) -> &str {
        self.provider.name()
    }

    /// Returns model information for this backend
    pub fn model_info(&self) -> Option<String> {
        Some(self.model.clone())
    }
}

/// Parses the detection result from submit_detection tool call arguments
fn parse_detection_from_tool_call(
    arguments: &serde_json::Value,
) -> Result<DetectionResult, BackendError> {
    debug!("Parsing detection from tool call arguments");

    // Convert arguments to JSON string
    let json_str = serde_json::to_string(arguments).map_err(|e| BackendError::ParseError {
        message: format!("Failed to serialize detection: {}", e),
        context: format!("{:?}", arguments),
    })?;

    debug!("Detection JSON: {}", json_str);

    // Use existing parser
    parse_ollama_response(&json_str).map_err(|e| {
        error!("Failed to parse detection result: {}", e);
        BackendError::ParseError {
            message: format!("Failed to parse detection result: {}", e),
            context: json_str,
        }
    })
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

#[async_trait]
impl LLMBackend for GenAIBackend {
    async fn detect(&self, repo_path: PathBuf) -> Result<DetectionResult, BackendError> {
        self.detect(repo_path).await
    }

    fn name(&self) -> &str {
        self.name()
    }

    fn model_info(&self) -> Option<String> {
        self.model_info()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(backend.model, "qwen2.5-coder:7b");
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
