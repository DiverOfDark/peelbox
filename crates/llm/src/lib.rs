mod client;
pub mod embedded;
mod genai;
mod lazy;
mod mock;
mod recording;
mod selector;
pub mod test_context;
mod types;

pub use ::genai::adapter::AdapterKind;
pub use client::LLMClient;
pub use embedded::{
    ComputeDevice, EmbeddedClient, EmbeddedModel, HardwareCapabilities, HardwareDetector,
    ModelDownloader, ModelSelector,
};
pub use genai::GenAIClient;
pub use lazy::LazyLLMClient;
pub use mock::{MockLLMClient, MockResponse};
pub use peelbox_core::BackendError;
pub use recording::{RecordedExchange, RecordedRequest, RecordingLLMClient, RecordingMode};
pub use selector::{select_llm_client, SelectedClient};
pub use test_context::TestContext;
pub use types::{ChatMessage, LLMRequest, LLMResponse, MessageRole, ToolCall, ToolDefinition};
