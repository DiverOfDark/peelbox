//! Build system detection
//!
//! This module handles repository analysis and build system detection,
//! including context gathering and result processing.

pub mod analyzer;
pub mod jumpstart;
pub mod prompt;
pub mod service;
pub mod tools;
pub mod types;

// Re-export commonly used types
pub use analyzer::{AnalysisError, AnalyzerConfig, RepositoryAnalyzer};
pub use jumpstart::{JumpstartContext, JumpstartScanner};
pub use prompt::SYSTEM_PROMPT;
pub use service::{DetectionService, ServiceError};
pub use types::{GitInfo, RepositoryContext};
