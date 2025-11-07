//! AI backend integrations
//!
//! This module provides abstractions and implementations for various LLM backends
//! that power the build system detection capabilities.

pub mod backend;
pub mod ollama;
pub mod lm_studio;

// Re-export commonly used types
pub use backend::{BackendConfig, BackendError, LLMBackend};
pub use ollama::OllamaClient;
pub use lm_studio::LMStudioClient;
