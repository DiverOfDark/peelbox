//! OpenAI-compatible HTTP client for local LLM inference
//!
//! This module provides an HTTP client compatible with OpenAI's API format,
//! supporting multiple local LLM services like Ollama and LM Studio that expose
//! OpenAI-compatible endpoints. This unified approach reduces code duplication
//! while supporting multiple backends.
//!
//! # Example
//!
//! ```no_run
//! use aipack::ai::backend::LLMBackend;
//! use aipack::ai::openai_compatible::OpenAICompatibleClient;
//! use aipack::detection::types::RepositoryContext;
//! use std::path::PathBuf;
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = OpenAICompatibleClient::new(
//!     "http://localhost:11434".to_string(),
//!     "qwen2.5-coder:7b".to_string(),
//! );
//!
//! // Check if service is available
//! if client.health_check().await? {
//!     let context = RepositoryContext::minimal(
//!         PathBuf::from("/path/to/repo"),
//!         "repo/\n├── Cargo.toml\n└── src/".to_string(),
//!     );
//!
//!     let result = client.detect(context).await?;
//!     println!("Detected: {}", result.build_system);
//! }
//! # Ok(())
//! # }
//! ```

use crate::ai::backend::{BackendError, LLMBackend};
use crate::detection::prompt::PromptBuilder;
use crate::detection::response::parse_ollama_response;
use crate::detection::types::{DetectionResult, RepositoryContext};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Default request timeout for API calls
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// OpenAI-compatible client for local and remote LLM inference
///
/// This client communicates with OpenAI-compatible API endpoints, including
/// local services like Ollama and LM Studio. It implements the `LLMBackend` trait
/// to provide a consistent interface with other backends.
///
/// # Configuration
///
/// - **endpoint**: API endpoint (e.g., "http://localhost:11434" for Ollama, "http://localhost:8000" for LM Studio)
/// - **model**: Model name (e.g., "qwen2.5-coder:7b")
/// - **timeout**: Request timeout duration
///
/// # Thread Safety
///
/// This client is thread-safe and can be shared across threads using `Arc`.
pub struct OpenAICompatibleClient {
    /// API endpoint URL
    endpoint: String,

    /// Model name to use for inference
    model: String,

    /// Shared HTTP client with connection pooling
    http_client: Client,

    /// Request timeout duration
    timeout: Duration,
}

impl OpenAICompatibleClient {
    /// Creates a new OpenAI-compatible client with default timeout
    ///
    /// # Arguments
    ///
    /// * `endpoint` - API endpoint (e.g., "http://localhost:11434" for Ollama)
    /// * `model` - Model name (e.g., "qwen2.5-coder:7b")
    ///
    /// # Example
    ///
    /// ```
    /// use aipack::ai::openai_compatible::OpenAICompatibleClient;
    ///
    /// let client = OpenAICompatibleClient::new(
    ///     "http://localhost:11434".to_string(),
    ///     "qwen2.5-coder:7b".to_string(),
    /// );
    /// ```
    pub fn new(endpoint: String, model: String) -> Self {
        Self::with_timeout(endpoint, model, Duration::from_secs(DEFAULT_TIMEOUT_SECS))
    }

    /// Creates a new OpenAI-compatible client with custom timeout
    ///
    /// # Arguments
    ///
    /// * `endpoint` - API endpoint
    /// * `model` - Model name
    /// * `timeout` - Request timeout duration
    ///
    /// # Example
    ///
    /// ```
    /// use aipack::ai::openai_compatible::OpenAICompatibleClient;
    /// use std::time::Duration;
    ///
    /// let client = OpenAICompatibleClient::with_timeout(
    ///     "http://localhost:11434".to_string(),
    ///     "qwen2.5-coder:7b".to_string(),
    ///     Duration::from_secs(60),
    /// );
    /// ```
    pub fn with_timeout(endpoint: String, model: String, timeout: Duration) -> Self {
        let http_client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            endpoint,
            model,
            http_client,
            timeout,
        }
    }

    /// Checks if the OpenAI-compatible server is available and healthy
    ///
    /// This method makes a lightweight request to the `/v1/models` endpoint
    /// to verify that the service is running and accessible.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if server is healthy, `Ok(false)` if unreachable,
    /// or `Err` if there's a connection error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::ai::openai_compatible::OpenAICompatibleClient;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = OpenAICompatibleClient::new(
    ///     "http://localhost:11434".to_string(),
    ///     "qwen2.5-coder:7b".to_string(),
    /// );
    ///
    /// if client.health_check().await? {
    ///     println!("Service is available");
    /// } else {
    ///     println!("Service is not responding");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health_check(&self) -> Result<bool, BackendError> {
        let url = format!("{}/v1/models", self.endpoint);

        debug!("Checking service health at {}", url);

        match self.http_client.get(&url).send().await {
            Ok(response) => {
                let is_healthy = response.status().is_success();
                if is_healthy {
                    info!("Service health check successful");
                } else {
                    warn!("Service health check failed with status: {}", response.status());
                }
                Ok(is_healthy)
            }
            Err(e) => {
                if e.is_timeout() {
                    warn!("Service health check timed out");
                    Ok(false)
                } else if e.is_connect() {
                    warn!("Cannot connect to service at {}", self.endpoint);
                    Ok(false)
                } else {
                    error!("Service health check error: {}", e);
                    Err(BackendError::NetworkError {
                        message: format!("Health check failed: {}", e),
                    })
                }
            }
        }
    }

    /// Internal method to call the OpenAI-compatible API
    async fn generate(&self, prompt: String) -> Result<String, BackendError> {
        let url = format!("{}/v1/chat/completions", self.endpoint);

        // Build OpenAI-compatible message format
        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are an expert at analyzing repository structure and detecting build systems. \
                        Respond with valid JSON only."
                        .to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: prompt,
                },
            ],
            temperature: Some(0.3),
            top_p: Some(0.9),
            max_tokens: Some(512),
            stream: Some(false),
        };

        debug!(
            "Sending request to service: prompt_length={}",
            request.messages[1].content.len()
        );

        let start = Instant::now();

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", "Bearer dummy-api-key")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    error!("Service request timed out after {:?}", self.timeout);
                    BackendError::TimeoutError {
                        seconds: self.timeout.as_secs(),
                    }
                } else if e.is_connect() {
                    error!("Cannot connect to service at {}", self.endpoint);
                    BackendError::NetworkError {
                        message: format!("Connection failed: {}", e),
                    }
                } else {
                    error!("Service request error: {}", e);
                    BackendError::NetworkError {
                        message: format!("Request failed: {}", e),
                    }
                }
            })?;

        let elapsed = start.elapsed();

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            error!("Service API returned error status {}: {}", status, body);

            return Err(BackendError::ApiError {
                message: format!("HTTP {}: {}", status, body),
                status_code: Some(status.as_u16()),
            });
        }

        let api_response: OpenAIResponse = response.json().await.map_err(|e| {
            error!("Failed to parse service response: {}", e);
            BackendError::InvalidResponse {
                message: format!("JSON parse error: {}", e),
                raw_response: None,
            }
        })?;

        info!(
            "Service generation completed in {:.2}s",
            elapsed.as_secs_f64()
        );

        debug!(
            "Service stats: prompt_tokens={}, completion_tokens={}",
            api_response
                .usage
                .as_ref()
                .map(|u| u.prompt_tokens)
                .unwrap_or(0),
            api_response
                .usage
                .as_ref()
                .map(|u| u.completion_tokens)
                .unwrap_or(0),
        );

        // Extract the assistant's response
        let content = api_response
            .choices
            .first()
            .and_then(|choice| choice.message.as_ref())
            .map(|message| message.content.clone())
            .ok_or_else(|| BackendError::InvalidResponse {
                message: "No content in service response".to_string(),
                raw_response: None,
            })?;

        Ok(content)
    }
}

#[async_trait]
impl LLMBackend for OpenAICompatibleClient {
    /// Detects build system and generates commands from repository context
    ///
    /// This method constructs a prompt from the repository context, sends it to
    /// the OpenAI-compatible API, and parses the response.
    async fn detect(&self, context: RepositoryContext) -> Result<DetectionResult, BackendError> {
        info!(
            "Starting detection for repository: {}",
            context.repo_path.display()
        );

        // Build the prompt
        let prompt = PromptBuilder::build_detection_prompt(&context);
        debug!("Built prompt with {} characters", prompt.len());

        // Call API
        let response_text = self.generate(prompt).await?;
        debug!("Received response with {} characters", response_text.len());

        // Parse the response
        let mut result = parse_ollama_response(&response_text).map_err(|e| {
            error!("Failed to parse service response: {}", e);
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
        "openai-compatible"
    }

    fn model_info(&self) -> Option<String> {
        Some(format!("{} @ {}", self.model, self.endpoint))
    }
}

impl fmt::Debug for OpenAICompatibleClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAICompatibleClient")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// Message structure for OpenAI-compatible API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    /// Role: "system", "user", or "assistant"
    role: String,
    /// Message content
    content: String,
}

/// Request structure for OpenAI-compatible chat completions API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIRequest {
    /// Model identifier
    model: String,
    /// Array of messages in conversation
    messages: Vec<Message>,
    /// Sampling temperature (0.0-2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    /// Nucleus sampling parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    /// Maximum tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    /// Whether to stream the response
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

/// Response structure from OpenAI-compatible chat completions API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIResponse {
    /// Response ID
    id: Option<String>,
    /// Object type
    object: Option<String>,
    /// Creation timestamp
    created: Option<i64>,
    /// Model used
    model: Option<String>,
    /// Array of completion choices
    choices: Vec<Choice>,
    /// Token usage statistics
    usage: Option<Usage>,
}

/// Completion choice from API response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Choice {
    /// Choice index
    index: Option<u32>,
    /// Stop reason
    finish_reason: Option<String>,
    /// Message content
    message: Option<Message>,
}

/// Token usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Usage {
    /// Number of prompt tokens
    prompt_tokens: u32,
    /// Number of completion tokens
    completion_tokens: u32,
    /// Total tokens
    total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = OpenAICompatibleClient::new(
            "http://localhost:11434".to_string(),
            "qwen2.5-coder:7b".to_string(),
        );
        assert_eq!(client.name(), "openai-compatible");
        assert!(client.model_info().is_some());
    }

    #[test]
    fn test_client_with_custom_timeout() {
        let timeout = Duration::from_secs(120);
        let client = OpenAICompatibleClient::with_timeout(
            "http://localhost:11434".to_string(),
            "qwen2.5-coder:7b".to_string(),
            timeout,
        );
        assert_eq!(client.timeout, timeout);
    }

    #[test]
    fn test_request_serialization() {
        let request = OpenAIRequest {
            model: "test-model".to_string(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are helpful.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: "Hello".to_string(),
                },
            ],
            temperature: Some(0.3),
            top_p: Some(0.9),
            max_tokens: Some(512),
            stream: Some(false),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"role\":\"system\""));
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"temperature\":0.3"));
    }

    #[test]
    fn test_response_parsing() {
        let response_json = r#"{
            "id": "test-id",
            "object": "chat.completion",
            "created": 1234567890,
            "model": "test-model",
            "choices": [{
                "index": 0,
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "Test response"
                }
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            }
        }"#;

        let response: OpenAIResponse = serde_json::from_str(response_json).unwrap();
        assert_eq!(response.choices.len(), 1);
        assert_eq!(
            response.choices[0]
                .message
                .as_ref()
                .unwrap()
                .content,
            "Test response"
        );
        assert_eq!(response.usage.unwrap().prompt_tokens, 10);
    }

    #[test]
    fn test_backend_trait_methods() {
        let client = OpenAICompatibleClient::new(
            "http://localhost:11434".to_string(),
            "qwen2.5-coder:7b".to_string(),
        );
        assert_eq!(client.name(), "openai-compatible");

        let model_info = client.model_info().unwrap();
        assert!(model_info.contains("qwen2.5-coder:7b"));
        assert!(model_info.contains("localhost:11434"));
    }

    #[test]
    fn test_debug_impl() {
        let client = OpenAICompatibleClient::new(
            "http://localhost:11434".to_string(),
            "qwen2.5-coder:7b".to_string(),
        );
        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("OpenAICompatibleClient"));
        assert!(debug_str.contains("localhost:11434"));
    }
}
