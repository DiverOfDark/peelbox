//! AI backend types and errors
//!
//! This module defines types for working with multiple LLM providers
//! including provider selection and error handling.

pub mod genai_backend;

// Re-export commonly used types
pub use genai::adapter::AdapterKind;
pub use genai_backend::{AdapterKindExt, BackendError};
