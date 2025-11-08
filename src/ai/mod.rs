//! AI backend integrations
//!
//! This module provides abstractions and implementations for various LLM backends
//! that power the build system detection capabilities.

pub mod backend;
pub mod genai_backend;

// Re-export commonly used types
pub use backend::{BackendError, LLMBackend};
pub use genai_backend::{GenAIBackend, Provider};
