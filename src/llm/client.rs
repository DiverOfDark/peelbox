use super::error::BackendError;
use super::types::{LLMRequest, LLMResponse};
use async_trait::async_trait;

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat(&self, request: LLMRequest) -> Result<LLMResponse, BackendError>;

    fn name(&self) -> &str;

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
