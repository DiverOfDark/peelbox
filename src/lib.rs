//! aipack - AI-powered buildkit frontend for container images
//!
//! This library provides intelligent build system detection and command generation
//! using Large Language Models (LLMs). It analyzes repository structure and generates
//! appropriate build commands for creating optimized container images.
//!
//! # Core Concepts
//!
//! - **LLM Backends**: Pluggable AI providers (Claude, OpenAI, local models) that
//!   analyze repositories and generate build commands
//! - **Detection**: Process of analyzing repository structure, configuration files,
//!   and metadata to identify the build system
//! - **Repository Context**: Structured information about a repository including
//!   file tree, key files, and Git metadata
//!
//! # Example Usage
//!
//! ```ignore
//! use aipack::{LLMBackend, RepositoryContext, DetectionResult};
//! use std::path::PathBuf;
//!
//! async fn detect_build_system(
//!     backend: impl LLMBackend,
//!     repo_path: PathBuf,
//! ) -> Result<DetectionResult, Box<dyn std::error::Error>> {
//!     // Gather repository context
//!     let context = RepositoryContext::minimal(repo_path, "file tree...".to_string());
//!
//!     // Detect build system using LLM
//!     let result = backend.detect(context).await?;
//!
//!     // Use the detected commands
//!     println!("Build: {}", result.build_command);
//!     println!("Test: {}", result.test_command);
//!
//!     Ok(result)
//! }
//! ```
//!
//! # Project Structure
//!
//! - [`ai`]: LLM backend implementations and abstractions
//! - [`detection`]: Repository analysis and detection types
//!
//! # Features
//!
//! - Multi-backend LLM support (Claude, OpenAI, local models)
//! - Comprehensive repository analysis
//! - Confidence scoring for detection results
//! - Warning generation for potential issues
//! - Git integration for additional context

// Public modules
pub mod ai;
pub mod cli;
pub mod config;
pub mod detection;
pub mod util;

// Re-export key types for convenient access
pub use ai::backend::{BackendError, LLMBackend};
pub use ai::genai_backend::{GenAIBackend, Provider};
pub use config::{AipackConfig, ConfigError};
pub use detection::analyzer::{AnalysisError, AnalyzerConfig, RepositoryAnalyzer};
pub use detection::service::{DetectionService, ServiceError};
pub use detection::types::{DetectionResult, GitInfo, RepositoryContext};
pub use util::{init_default, init_from_env, init_logging, LoggingConfig};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_exists() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_name_is_aipack() {
        assert_eq!(NAME, "aipack");
    }
}
