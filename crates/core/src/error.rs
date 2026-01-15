use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendError {
    ApiError {
        message: String,
        status_code: Option<u16>,
    },
    AuthenticationError {
        message: String,
    },
    TimeoutError {
        seconds: u64,
    },
    RateLimitError {
        retry_after: Option<u64>,
    },
    InvalidResponse {
        message: String,
        raw_response: Option<String>,
    },
    ConfigurationError {
        message: String,
    },
    NetworkError {
        message: String,
    },
    ParseError {
        message: String,
        context: String,
    },
    Other {
        message: String,
    },
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
