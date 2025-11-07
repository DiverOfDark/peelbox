//! Ollama HTTP client for local LLM inference
//!
//! This module provides an HTTP client for the Ollama API, enabling local
//! LLM inference for build system detection. Ollama supports various models
//! including Qwen, Llama, Mistral, and others.
//!
//! # Example
//!
//! ```no_run
//! use aipack::ai::backend::LLMBackend;
//! use aipack::ai::ollama::OllamaClient;
//! use aipack::detection::types::RepositoryContext;
//! use std::path::PathBuf;
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = OllamaClient::with_timeout(
//!     "http://localhost:11434".to_string(),
//!     "qwen:7b".to_string(),
//!     Duration::from_secs(60),
//! );
//!
//! // Check if Ollama is available
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

/// Default request timeout for Ollama API calls
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Ollama client for local LLM inference
///
/// This client communicates with a local Ollama server to perform build system
/// detection using various open-source LLMs. It implements the `LLMBackend` trait
/// to provide a consistent interface with other backends.
///
/// # Configuration
///
/// - **endpoint**: Ollama API endpoint (e.g., "http://localhost:11434")
/// - **model**: Model name (e.g., "qwen:7b", "llama2", "mistral")
/// - **timeout**: Request timeout duration
///
/// # Thread Safety
///
/// This client is thread-safe and can be shared across threads using `Arc`.
pub struct OllamaClient {
    /// Ollama API endpoint URL
    endpoint: String,

    /// Model name to use for inference
    model: String,

    /// Shared HTTP client with connection pooling
    http_client: Client,

    /// Request timeout duration
    timeout: Duration,
}

impl OllamaClient {
    /// Creates a new Ollama client with default timeout
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Ollama API endpoint (e.g., "http://localhost:11434")
    /// * `model` - Model name (e.g., "qwen:7b")
    ///
    /// # Example
    ///
    /// ```
    /// use aipack::ai::ollama::OllamaClient;
    ///
    /// let client = OllamaClient::new(
    ///     "http://localhost:11434".to_string(),
    ///     "qwen:7b".to_string(),
    /// );
    /// ```
    pub fn new(endpoint: String, model: String) -> Self {
        Self::with_timeout(endpoint, model, Duration::from_secs(DEFAULT_TIMEOUT_SECS))
    }

    /// Creates a new Ollama client with custom timeout
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Ollama API endpoint
    /// * `model` - Model name
    /// * `timeout` - Request timeout duration
    ///
    /// # Example
    ///
    /// ```
    /// use aipack::ai::ollama::OllamaClient;
    /// use std::time::Duration;
    ///
    /// let client = OllamaClient::with_timeout(
    ///     "http://localhost:11434".to_string(),
    ///     "qwen:7b".to_string(),
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

    /// Checks if the Ollama server is available and healthy
    ///
    /// This method makes a lightweight request to the `/api/tags` endpoint
    /// to verify that Ollama is running and accessible.
    ///
    /// # Returns
    ///
    /// `Ok(true)` if Ollama is healthy, `Ok(false)` if unreachable,
    /// or `Err` if there's a connection error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::ai::ollama::OllamaClient;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = OllamaClient::new(
    ///     "http://localhost:11434".to_string(),
    ///     "qwen:7b".to_string(),
    /// );
    ///
    /// if client.health_check().await? {
    ///     println!("Ollama is available");
    /// } else {
    ///     println!("Ollama is not responding");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn health_check(&self) -> Result<bool, BackendError> {
        let url = format!("{}/api/tags", self.endpoint);

        debug!("Checking Ollama health at {}", url);

        match self.http_client.get(&url).send().await {
            Ok(response) => {
                let is_healthy = response.status().is_success();
                if is_healthy {
                    info!("Ollama health check successful");
                } else {
                    warn!(
                        "Ollama health check failed with status: {}",
                        response.status()
                    );
                }
                Ok(is_healthy)
            }
            Err(e) => {
                if e.is_timeout() {
                    warn!("Ollama health check timed out");
                    Ok(false)
                } else if e.is_connect() {
                    warn!("Cannot connect to Ollama at {}", self.endpoint);
                    Ok(false)
                } else {
                    error!("Ollama health check error: {}", e);
                    Err(BackendError::NetworkError {
                        message: format!("Health check failed: {}", e),
                    })
                }
            }
        }
    }

    /// Internal method to call the Ollama generate API
    async fn generate(&self, prompt: String) -> Result<String, BackendError> {
        let url = format!("{}/api/generate", self.endpoint);

        let request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            temperature: Some(0.3),
            top_p: Some(0.9),
            num_predict: Some(512),
        };

        debug!(
            "Sending request to Ollama: model={}, prompt_length={}",
            self.model,
            request.prompt.len()
        );

        let start = Instant::now();

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    error!("Ollama request timed out after {:?}", self.timeout);
                    BackendError::TimeoutError {
                        seconds: self.timeout.as_secs(),
                    }
                } else if e.is_connect() {
                    error!("Cannot connect to Ollama at {}", self.endpoint);
                    BackendError::NetworkError {
                        message: format!("Connection failed: {}", e),
                    }
                } else {
                    error!("Ollama request error: {}", e);
                    BackendError::NetworkError {
                        message: format!("Request failed: {}", e),
                    }
                }
            })?;

        let elapsed = start.elapsed();

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            error!("Ollama API returned error status {}: {}", status, body);

            // Check for specific error cases
            if status.as_u16() == 404 && body.contains("model") {
                return Err(BackendError::Other {
                    message: format!(
                        "Model '{}' not found. Please pull it with: ollama pull {}",
                        self.model, self.model
                    ),
                });
            }

            return Err(BackendError::ApiError {
                message: format!("HTTP {}: {}", status, body),
                status_code: Some(status.as_u16()),
            });
        }

        let ollama_response: OllamaResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Ollama response: {}", e);
            BackendError::InvalidResponse {
                message: format!("JSON parse error: {}", e),
                raw_response: None,
            }
        })?;

        if !ollama_response.done {
            warn!("Ollama response indicates incomplete generation");
        }

        info!(
            "Ollama generation completed in {:.2}s (model={})",
            elapsed.as_secs_f64(),
            self.model
        );

        debug!(
            "Ollama stats: prompt_tokens={}, eval_tokens={}, total_duration={:?}",
            ollama_response.prompt_eval_count.unwrap_or(0),
            ollama_response.eval_count.unwrap_or(0),
            ollama_response.total_duration
        );

        Ok(ollama_response.response)
    }
}

#[async_trait]
impl LLMBackend for OllamaClient {
    /// Detects build system and generates commands from repository context
    ///
    /// This method constructs a prompt from the repository context, sends it to
    /// Ollama, and parses the response into a structured `DetectionResult`.
    ///
    /// # Arguments
    ///
    /// * `context` - Repository information including file tree and key files
    ///
    /// # Returns
    ///
    /// A `DetectionResult` containing detected build system and commands
    ///
    /// # Errors
    ///
    /// Returns `BackendError` if:
    /// - Ollama is unreachable
    /// - The request times out
    /// - The response cannot be parsed
    /// - The model is not found
    async fn detect(&self, context: RepositoryContext) -> Result<DetectionResult, BackendError> {
        info!(
            "Starting detection for repository: {}",
            context.repo_path.display()
        );

        // Build the prompt
        let prompt = PromptBuilder::build_detection_prompt(&context);
        debug!("Built prompt with {} characters", prompt.len());

        // Call Ollama API
        let response_text = self.generate(prompt).await?;
        debug!("Received response with {} characters", response_text.len());

        // Parse the response
        let mut result = parse_ollama_response(&response_text).map_err(|e| {
            error!("Failed to parse Ollama response: {}", e);
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
        "ollama"
    }

    fn model_info(&self) -> Option<String> {
        Some(format!("{} @ {}", self.model, self.endpoint))
    }
}

impl fmt::Debug for OllamaClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OllamaClient")
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// Request structure for Ollama generate API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaRequest {
    /// Model name to use for generation
    model: String,

    /// Prompt text to send to the model
    prompt: String,

    /// Whether to stream the response (false for this use case)
    stream: bool,

    /// Temperature for sampling (0.0 = deterministic, 1.0 = creative)
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,

    /// Top-p (nucleus) sampling parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,

    /// Maximum number of tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

/// Response structure from Ollama generate API
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaResponse {
    /// Model that was used
    model: String,

    /// Timestamp when the response was created
    created_at: String,

    /// Generated response text
    response: String,

    /// Whether generation is complete
    done: bool,

    /// Total duration in nanoseconds (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    total_duration: Option<u64>,

    /// Model load duration in nanoseconds (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    load_duration: Option<u64>,

    /// Number of tokens in the prompt (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_eval_count: Option<u32>,

    /// Number of tokens generated (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_count: Option<u32>,

    /// Evaluation duration in nanoseconds (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_duration: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_client_creation() {
        let client = OllamaClient::new("http://localhost:11434".to_string(), "qwen:7b".to_string());

        assert_eq!(client.endpoint, "http://localhost:11434");
        assert_eq!(client.model, "qwen:7b");
        assert_eq!(client.timeout, Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn test_ollama_client_with_custom_timeout() {
        let client = OllamaClient::with_timeout(
            "http://localhost:11434".to_string(),
            "qwen:7b".to_string(),
            Duration::from_secs(60),
        );

        assert_eq!(client.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_backend_trait_methods() {
        let client = OllamaClient::new("http://localhost:11434".to_string(), "qwen:7b".to_string());

        assert_eq!(client.name(), "ollama");
        assert!(client.model_info().is_some());
        assert!(client
            .model_info()
            .unwrap()
            .contains("qwen:7b @ http://localhost:11434"));
    }

    #[test]
    fn test_ollama_request_serialization() {
        let request = OllamaRequest {
            model: "qwen:7b".to_string(),
            prompt: "test prompt".to_string(),
            stream: false,
            temperature: Some(0.3),
            top_p: Some(0.9),
            num_predict: Some(512),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"qwen:7b\""));
        assert!(json.contains("\"prompt\":\"test prompt\""));
        assert!(json.contains("\"stream\":false"));
        assert!(json.contains("\"temperature\":0.3"));
    }

    #[test]
    fn test_ollama_response_deserialization() {
        let json = r#"{
            "model": "qwen:7b",
            "created_at": "2024-01-01T00:00:00Z",
            "response": "test response",
            "done": true,
            "total_duration": 1000000,
            "prompt_eval_count": 10,
            "eval_count": 20
        }"#;

        let response: OllamaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.model, "qwen:7b");
        assert_eq!(response.response, "test response");
        assert!(response.done);
        assert_eq!(response.prompt_eval_count, Some(10));
        assert_eq!(response.eval_count, Some(20));
    }

    #[test]
    fn test_ollama_response_minimal() {
        let json = r#"{
            "model": "qwen:7b",
            "created_at": "2024-01-01T00:00:00Z",
            "response": "test",
            "done": true
        }"#;

        let response: OllamaResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.model, "qwen:7b");
        assert!(response.total_duration.is_none());
        assert!(response.prompt_eval_count.is_none());
    }

    #[tokio::test]
    async fn test_health_check_unreachable() {
        // Use a non-existent endpoint
        let client = OllamaClient::with_timeout(
            "http://localhost:59999".to_string(),
            "qwen:7b".to_string(),
            Duration::from_millis(100),
        );

        let result = client.health_check().await;
        // Should return Ok(false) for unreachable endpoint
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    // Note: Integration tests with actual Ollama server are in tests/ollama_integration.rs
}
