//! GenAI backend types and errors
//!
//! This module defines extension methods for AdapterKind
//! and BackendError for AI backend error handling.

use genai::adapter::AdapterKind;
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

/// Extension trait for AdapterKind providing aipack-specific helper methods
pub trait AdapterKindExt {
    /// Returns the human-readable name of the provider
    fn name(&self) -> &'static str;

    /// Reads custom endpoint from environment variable
    fn custom_endpoint(&self) -> Option<String>;

    /// Returns the environment variable name for API key
    fn api_key_env_var(&self) -> &'static str;
}

impl AdapterKindExt for AdapterKind {
    fn name(&self) -> &'static str {
        match self {
            AdapterKind::Ollama => "Ollama",
            AdapterKind::Anthropic => "Claude",
            AdapterKind::OpenAI => "OpenAI",
            AdapterKind::Gemini => "Gemini",
            AdapterKind::Xai => "Grok",
            AdapterKind::Groq => "Groq",
            _ => self.as_str(),
        }
    }

    fn custom_endpoint(&self) -> Option<String> {
        match self {
            AdapterKind::Ollama => std::env::var("OLLAMA_HOST").ok(),
            AdapterKind::OpenAI => std::env::var("OPENAI_API_BASE").ok(),
            AdapterKind::Anthropic => std::env::var("ANTHROPIC_BASE_URL").ok(),
            AdapterKind::Gemini => std::env::var("GOOGLE_API_BASE_URL").ok(),
            AdapterKind::Xai => std::env::var("XAI_BASE_URL").ok(),
            AdapterKind::Groq => std::env::var("GROQ_BASE_URL").ok(),
            _ => None,
        }
    }

    fn api_key_env_var(&self) -> &'static str {
        match self {
            AdapterKind::Ollama => "",
            AdapterKind::OpenAI => "OPENAI_API_KEY",
            AdapterKind::Anthropic => "ANTHROPIC_API_KEY",
            AdapterKind::Gemini => "GOOGLE_API_KEY",
            AdapterKind::Xai => "XAI_API_KEY",
            AdapterKind::Groq => "GROQ_API_KEY",
            _ => "",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_kind_name() {
        assert_eq!(AdapterKind::Ollama.name(), "Ollama");
        assert_eq!(AdapterKind::Anthropic.name(), "Claude");
        assert_eq!(AdapterKind::OpenAI.name(), "OpenAI");
    }

    #[test]
    fn test_adapter_kind_display() {
        assert_eq!(format!("{}", AdapterKind::Ollama), "Ollama");
        assert_eq!(format!("{}", AdapterKind::Anthropic), "Anthropic");
        assert_eq!(format!("{}", AdapterKind::OpenAI), "OpenAI");
    }

    #[test]
    fn test_adapter_kind_api_key_env() {
        assert_eq!(AdapterKind::Ollama.api_key_env_var(), "");
        assert_eq!(AdapterKind::Anthropic.api_key_env_var(), "ANTHROPIC_API_KEY");
        assert_eq!(AdapterKind::OpenAI.api_key_env_var(), "OPENAI_API_KEY");
    }
}
