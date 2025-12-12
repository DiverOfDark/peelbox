//! GenAI backend types and errors
//!
//! This module defines BackendError for AI backend error handling.

use serde::{Deserialize, Serialize};
use std::fmt;

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

    /// The LLM response could not be parsed into a UniversalBuild
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
