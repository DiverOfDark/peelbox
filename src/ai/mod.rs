//! AI backend integrations
//!
//! This module provides a multi-provider GenAI backend that supports Claude, OpenAI,
//! Gemini, Ollama, Grok, and Groq for build system detection.

pub mod genai_backend;

// Re-export commonly used types
pub use genai_backend::{BackendError, GenAIBackend, Provider};
