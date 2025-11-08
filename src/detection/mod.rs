//! Build system detection
//!
//! This module handles repository analysis and build system detection,
//! including context gathering and result processing.

pub mod analyzer;
pub mod prompt;
pub mod response;
pub mod service;
pub mod tools;
pub mod types;

// Re-export commonly used types
pub use analyzer::{AnalysisError, AnalyzerConfig, RepositoryAnalyzer};
pub use prompt::PromptBuilder;
pub use response::{parse_ollama_response, validate_detection_result, ParseError};
pub use service::{DetectionService, ServiceError};
pub use types::{DetectionResult, GitInfo, RepositoryContext};
