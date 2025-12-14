use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool_response(call_id: impl Into<String>, result: serde_json::Value) -> Self {
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());

        Self {
            role: MessageRole::Tool,
            content,
            tool_calls: None,
            tool_call_id: Some(call_id.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct LLMRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stop_sequences: Option<Vec<String>>,
}

impl LLMRequest {
    pub fn new(messages: Vec<ChatMessage>) -> Self {
        Self {
            messages,
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            stop_sequences: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub content: String,
    pub tool_call: Option<ToolCall>,
    #[serde(
        serialize_with = "serialize_duration",
        deserialize_with = "deserialize_duration"
    )]
    pub response_time: Duration,
}

fn serialize_duration<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(duration.as_millis() as u64)
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let millis = u64::deserialize(deserializer)?;
    Ok(Duration::from_millis(millis))
}

impl LLMResponse {
    pub fn text(content: impl Into<String>, response_time: Duration) -> Self {
        Self {
            content: content.into(),
            tool_call: None,
            response_time,
        }
    }

    pub fn with_tool_call(
        content: impl Into<String>,
        tool_call: ToolCall,
        response_time: Duration,
    ) -> Self {
        Self {
            content: content.into(),
            tool_call: Some(tool_call),
            response_time,
        }
    }

    pub fn has_tool_call(&self) -> bool {
        self.tool_call.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_creation() {
        let system = ChatMessage::system("You are a helpful assistant");
        assert_eq!(system.role, MessageRole::System);
        assert_eq!(system.content, "You are a helpful assistant");

        let user = ChatMessage::user("Hello");
        assert_eq!(user.role, MessageRole::User);

        let assistant = ChatMessage::assistant("Hi there!");
        assert_eq!(assistant.role, MessageRole::Assistant);
    }

    #[test]
    fn test_tool_response() {
        let response =
            ChatMessage::tool_response("call_123", serde_json::json!("File contents here"));
        assert_eq!(response.role, MessageRole::Tool);
        assert_eq!(response.tool_call_id, Some("call_123".to_string()));
    }

    #[test]
    fn test_assistant_with_tools() {
        let tool_call = ToolCall {
            call_id: "call_1".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({"path": "Cargo.toml"}),
        };

        let msg = ChatMessage::assistant_with_tools("Let me check that file", vec![tool_call]);
        assert!(msg.tool_calls.is_some());
        assert_eq!(msg.tool_calls.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_llm_request_builder() {
        let request = LLMRequest::new(vec![ChatMessage::user("Hello")])
            .with_temperature(0.7)
            .with_max_tokens(1024)
            .with_stop_sequences(vec!["</end>".to_string()]);

        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(1024));
        assert!(request.stop_sequences.is_some());
    }

    #[test]
    fn test_llm_response() {
        let response = LLMResponse::text("Hello!", Duration::from_millis(100));
        assert!(!response.has_tool_call());

        let with_tool = LLMResponse::with_tool_call(
            "Calling tool",
            ToolCall {
                call_id: "1".to_string(),
                name: "test".to_string(),
                arguments: serde_json::json!({}),
            },
            Duration::from_millis(50),
        );
        assert!(with_tool.has_tool_call());
    }
}
