use super::client::LLMClient;
use super::types::{LLMRequest, LLMResponse, ToolCall};
use crate::ai::error::BackendError;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

pub struct MockLLMClient {
    responses: Mutex<VecDeque<MockResponse>>,
    name: String,
}

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub error: Option<BackendError>,
}

impl MockResponse {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
            error: None,
        }
    }

    pub fn with_tool_calls(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            content: content.into(),
            tool_calls,
            error: None,
        }
    }

    pub fn error(error: BackendError) -> Self {
        Self {
            content: String::new(),
            tool_calls: Vec::new(),
            error: Some(error),
        }
    }
}

impl MockLLMClient {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            name: "MockLLM".to_string(),
        }
    }

    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            name: name.into(),
        }
    }

    pub fn add_response(&self, response: MockResponse) {
        self.responses.lock().unwrap().push_back(response);
    }

    pub fn add_responses(&self, responses: impl IntoIterator<Item = MockResponse>) {
        let mut queue = self.responses.lock().unwrap();
        for response in responses {
            queue.push_back(response);
        }
    }

    pub fn remaining_responses(&self) -> usize {
        self.responses.lock().unwrap().len()
    }

    pub fn read_file_call(call_id: impl Into<String>, path: impl Into<String>) -> ToolCall {
        ToolCall {
            call_id: call_id.into(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({ "path": path.into() }),
        }
    }

    pub fn list_files_call(call_id: impl Into<String>, path: impl Into<String>) -> ToolCall {
        ToolCall {
            call_id: call_id.into(),
            name: "list_files".to_string(),
            arguments: serde_json::json!({ "path": path.into() }),
        }
    }

    pub fn get_best_practices_call(
        call_id: impl Into<String>,
        language: impl Into<String>,
        build_system: impl Into<String>,
    ) -> ToolCall {
        ToolCall {
            call_id: call_id.into(),
            name: "get_best_practices".to_string(),
            arguments: serde_json::json!({
                "language": language.into(),
                "build_system": build_system.into()
            }),
        }
    }

    pub fn submit_detection_call(
        call_id: impl Into<String>,
        detection: serde_json::Value,
    ) -> ToolCall {
        ToolCall {
            call_id: call_id.into(),
            name: "submit_detection".to_string(),
            arguments: detection,
        }
    }
}

impl Default for MockLLMClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMClient for MockLLMClient {
    async fn chat(&self, _request: LLMRequest) -> Result<LLMResponse, BackendError> {
        let response =
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| BackendError::Other {
                    message: "MockLLMClient: No more responses in queue".to_string(),
                })?;

        // Return error if configured
        if let Some(error) = response.error {
            return Err(error);
        }

        // Take first tool call only
        let tool_call = response.tool_calls.into_iter().next();

        if let Some(tc) = tool_call {
            Ok(LLMResponse::with_tool_call(
                response.content,
                tc,
                Duration::from_millis(10),
            ))
        } else {
            Ok(LLMResponse::text(response.content, Duration::from_millis(10)))
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn model_info(&self) -> Option<String> {
        Some("mock-model".to_string())
    }
}

impl std::fmt::Debug for MockLLMClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockLLMClient")
            .field("name", &self.name)
            .field("remaining_responses", &self.remaining_responses())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_client_basic() {
        let client = MockLLMClient::new();
        client.add_response(MockResponse::text("Hello!"));

        let response = client.chat(LLMRequest::new(vec![])).await.unwrap();

        assert_eq!(response.content, "Hello!");
        assert!(response.tool_call.is_none());
    }

    #[tokio::test]
    async fn test_mock_client_with_tool_calls() {
        let client = MockLLMClient::new();

        let tool_call = MockLLMClient::read_file_call("call_1", "Cargo.toml");
        client.add_response(MockResponse::with_tool_calls(
            "Let me read that file",
            vec![tool_call.clone()],
        ));

        let response = client.chat(LLMRequest::new(vec![])).await.unwrap();

        assert!(response.tool_call.is_some());
        assert_eq!(response.tool_call.unwrap().name, "read_file");
    }

    #[tokio::test]
    async fn test_mock_client_error() {
        let client = MockLLMClient::new();
        client.add_response(MockResponse::error(BackendError::TimeoutError {
            seconds: 30,
        }));

        let result = client.chat(LLMRequest::new(vec![])).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_client_no_responses() {
        let client = MockLLMClient::new();

        let result = client.chat(LLMRequest::new(vec![])).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_client_multiple_responses() {
        let client = MockLLMClient::new();
        client.add_responses(vec![
            MockResponse::text("First"),
            MockResponse::text("Second"),
            MockResponse::text("Third"),
        ]);

        assert_eq!(client.remaining_responses(), 3);

        let r1 = client.chat(LLMRequest::new(vec![])).await.unwrap();
        assert_eq!(r1.content, "First");

        let r2 = client.chat(LLMRequest::new(vec![])).await.unwrap();
        assert_eq!(r2.content, "Second");

        assert_eq!(client.remaining_responses(), 1);
    }

    #[test]
    fn test_helper_methods() {
        let read_call = MockLLMClient::read_file_call("id1", "test.txt");
        assert_eq!(read_call.name, "read_file");

        let list_call = MockLLMClient::list_files_call("id2", "src");
        assert_eq!(list_call.name, "list_files");

        let bp_call = MockLLMClient::get_best_practices_call("id3", "rust", "cargo");
        assert_eq!(bp_call.name, "get_best_practices");
    }

    #[test]
    fn test_custom_name() {
        let client = MockLLMClient::with_name("TestClient");
        assert_eq!(client.name(), "TestClient");
    }
}
