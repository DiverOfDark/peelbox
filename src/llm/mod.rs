//! LLM client abstraction layer
//!
//! This module provides a trait-based abstraction for LLM communication,
//! allowing different backends (GenAI, Mock, Embedded) to be used interchangeably.

mod client;
mod genai;
mod types;

pub use client::LLMClient;
pub use genai::GenAIClient;
pub use types::{
    ChatMessage, LLMRequest, LLMResponse, MessageRole, ToolCall, ToolDefinition,
};
