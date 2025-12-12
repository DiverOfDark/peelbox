//! LLM client trait definition
//!
//! This module defines the core `LLMClient` trait that all LLM implementations
//! must implement to provide chat completion capabilities.

use super::types::{LLMRequest, LLMResponse};
use crate::ai::genai_backend::BackendError;
use async_trait::async_trait;

/// Trait for LLM chat completion clients
///
/// This trait abstracts the communication with LLM providers, allowing
/// different implementations (GenAI, Mock, Embedded) to be used interchangeably.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support async operations across threads.
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// Sends a chat request to the LLM and returns the response
    ///
    /// # Arguments
    ///
    /// * `request` - The chat request containing messages, tools, and options
    ///
    /// # Returns
    ///
    /// An `LLMResponse` containing the LLM's reply and any tool calls
    ///
    /// # Errors
    ///
    /// Returns `BackendError` if:
    /// - The request times out
    /// - Authentication fails
    /// - The API returns an error
    /// - Network connectivity issues
    async fn chat(&self, request: LLMRequest) -> Result<LLMResponse, BackendError>;

    /// Returns the name of this client for logging
    fn name(&self) -> &str;

    /// Returns optional model information
    fn model_info(&self) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    struct TestClient;

    #[async_trait]
    impl LLMClient for TestClient {
        async fn chat(&self, _request: LLMRequest) -> Result<LLMResponse, BackendError> {
            Ok(LLMResponse::text(
                "Test response",
                Duration::from_millis(10),
            ))
        }

        fn name(&self) -> &str {
            "TestClient"
        }
    }

    #[tokio::test]
    async fn test_client_trait() {
        let client = TestClient;
        assert_eq!(client.name(), "TestClient");
        assert!(client.model_info().is_none());
    }
}
