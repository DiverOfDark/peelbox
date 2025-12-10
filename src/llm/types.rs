//! LLM communication types
//!
//! This module defines the types used for LLM request/response communication,
//! independent of any specific provider implementation.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Role of a message in the conversation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System instructions
    System,
    /// User message
    User,
    /// Assistant (LLM) response
    Assistant,
    /// Tool response
    Tool,
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role of the message sender
    pub role: MessageRole,
    /// Text content of the message
    pub content: String,
    /// Tool calls made by the assistant (only for Assistant role)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID this message responds to (only for Tool role)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    /// Creates a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Creates a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Creates an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Creates an assistant message with tool calls
    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    /// Creates a tool response message
    pub fn tool_response(call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(call_id.into()),
        }
    }
}

/// A tool call requested by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub call_id: String,
    /// Name of the tool to call
    pub name: String,
    /// Arguments to pass to the tool (JSON object)
    pub arguments: serde_json::Value,
}

/// Definition of a tool available to the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Name of the tool
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// JSON Schema for the tool's parameters
    pub parameters: serde_json::Value,
}

/// Request to send to the LLM
#[derive(Debug, Clone)]
pub struct LLMRequest {
    /// Conversation messages
    pub messages: Vec<ChatMessage>,
    /// Tools available for the LLM to use
    pub tools: Vec<ToolDefinition>,
    /// Temperature for response generation (0.0 - 1.0)
    pub temperature: Option<f32>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Stop sequences to end generation
    pub stop_sequences: Option<Vec<String>>,
}

impl LLMRequest {
    /// Creates a new request with messages
    pub fn new(messages: Vec<ChatMessage>) -> Self {
        Self {
            messages,
            tools: Vec::new(),
            temperature: None,
            max_tokens: None,
            stop_sequences: None,
        }
    }

    /// Adds tools to the request
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    /// Sets the temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Sets the maximum tokens
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Sets stop sequences
    pub fn with_stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }
}

/// Response from the LLM
#[derive(Debug, Clone)]
pub struct LLMResponse {
    /// Text content of the response
    pub content: String,
    /// Tool calls requested by the LLM
    pub tool_calls: Vec<ToolCall>,
    /// Time taken for the request
    pub response_time: Duration,
}

impl LLMResponse {
    /// Creates a new response with just content
    pub fn text(content: impl Into<String>, response_time: Duration) -> Self {
        Self {
            content: content.into(),
            tool_calls: Vec::new(),
            response_time,
        }
    }

    /// Creates a new response with tool calls
    pub fn with_tool_calls(
        content: impl Into<String>,
        tool_calls: Vec<ToolCall>,
        response_time: Duration,
    ) -> Self {
        Self {
            content: content.into(),
            tool_calls,
            response_time,
        }
    }

    /// Returns true if the response contains tool calls
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
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
        let response = ChatMessage::tool_response("call_123", "File contents here");
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
        assert!(!response.has_tool_calls());

        let with_tools = LLMResponse::with_tool_calls(
            "Calling tool",
            vec![ToolCall {
                call_id: "1".to_string(),
                name: "test".to_string(),
                arguments: serde_json::json!({}),
            }],
            Duration::from_millis(50),
        );
        assert!(with_tools.has_tool_calls());
    }
}
