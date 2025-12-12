//! AI backend types and errors
//!
//! This module defines types for working with multiple LLM providers
//! including provider selection and error handling.

pub mod error;

// Re-export commonly used types
pub use error::BackendError;
pub use genai::adapter::AdapterKind;
