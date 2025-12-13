mod client;
pub mod embedded;
mod genai;
mod mock;
mod recording;
mod selector;
mod types;

pub use client::LLMClient;
pub use embedded::{
    ComputeDevice, EmbeddedClient, EmbeddedModel, HardwareCapabilities, HardwareDetector,
    ModelDownloader, ModelSelector,
};
pub use genai::GenAIClient;
pub use mock::{MockLLMClient, MockResponse};
pub use recording::{RecordedExchange, RecordedRequest, RecordingLLMClient, RecordingMode};
pub use selector::{select_llm_client, SelectedClient};
pub use types::{ChatMessage, LLMRequest, LLMResponse, MessageRole, ToolCall, ToolDefinition};
