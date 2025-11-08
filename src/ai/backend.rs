//! LLM Backend abstraction layer
//!
//! This module provides the core trait and types for implementing different LLM backends
//! (e.g., Claude, OpenAI, local models). All backends must implement the `LLMBackend` trait
//! to provide consistent detection capabilities.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::detection::types::{DetectionResult, RepositoryContext};

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

/// Core trait that all LLM backends must implement
///
/// This trait provides a uniform interface for detecting build systems and
/// generating build commands from repository context.
///
/// # Example
///
/// ```ignore
/// use aipack::ai::backend::LLMBackend;
/// use aipack::detection::types::RepositoryContext;
///
/// async fn detect_build_system(
///     backend: impl LLMBackend,
///     context: RepositoryContext,
/// ) -> Result<(), Box<dyn std::error::Error>> {
///     let result = backend.detect(context).await?;
///     println!("Detected: {} ({})", result.build_system, result.language);
///     println!("Build command: {}", result.build_command);
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait LLMBackend: Send + Sync {
    /// Detect build system and generate commands based on repository context
    ///
    /// This is the primary method that analyzes the repository structure and
    /// contents to determine the appropriate build system and commands.
    ///
    /// # Arguments
    ///
    /// * `context` - Repository information including file tree and key files
    ///
    /// # Returns
    ///
    /// A `DetectionResult` containing the identified build system, commands,
    /// and metadata about the detection process.
    ///
    /// # Errors
    ///
    /// Returns `BackendError` if the API call fails, times out, or the response
    /// cannot be parsed.
    async fn detect(&self, context: RepositoryContext) -> Result<DetectionResult, BackendError>;

    /// Returns the human-readable name of this backend
    ///
    /// # Example
    ///
    /// ```ignore
    /// assert_eq!(backend.name(), "Claude");
    /// ```
    fn name(&self) -> &str;

    /// Returns optional model information for this backend
    ///
    /// This can include the model version, variant, or other identifying
    /// information that might be useful for debugging or logging.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(info) = backend.model_info() {
    ///     println!("Using model: {}", info);
    /// }
    /// ```
    fn model_info(&self) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_error_display() {
        let error = BackendError::ApiError {
            message: "Test error".to_string(),
            status_code: Some(500),
        };
        assert!(error.to_string().contains("500"));
        assert!(error.to_string().contains("Test error"));
    }
}
