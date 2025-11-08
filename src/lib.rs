//! aipack - AI-powered buildkit frontend for container images
//!
//! This library provides intelligent build system detection and command generation
//! using Large Language Models (LLMs). It analyzes repository structure and generates
//! appropriate build commands for creating optimized container images.
//!
//! # Core Concepts
//!
//! - **GenAI Backend**: Multi-provider AI backend supporting Claude, OpenAI, Gemini,
//!   Ollama, Grok, and Groq for analyzing repositories and generating build commands
//! - **Detection**: Process of analyzing repository structure, configuration files,
//!   and metadata to identify the build system
//! - **Repository Context**: Structured information about a repository including
//!   file tree, key files, and Git metadata
//!
//! # Example Usage
//!
//! ```ignore
//! use aipack::{GenAIBackend, RepositoryContext, DetectionResult};
//! use std::path::PathBuf;
//! use std::sync::Arc;
//!
//! async fn detect_build_system(
//!     backend: Arc<GenAIBackend>,
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
//! - Multi-provider LLM support (Claude, OpenAI, Gemini, Ollama, Grok, Groq)
//! - Comprehensive repository analysis
//! - Confidence scoring for detection results
//! - Warning generation for potential issues
//! - Git integration for additional context

// Public modules
pub mod ai;
pub mod cli;
pub mod config;
pub mod detection;

// Re-export key types for convenient access
pub use ai::genai_backend::{BackendError, GenAIBackend, Provider};
pub use config::{AipackConfig, ConfigError};
pub use detection::analyzer::{AnalysisError, AnalyzerConfig, RepositoryAnalyzer};
pub use detection::service::{DetectionService, ServiceError};
pub use detection::types::{DetectionResult, GitInfo, RepositoryContext};

/// Initialize logging with default configuration
///
/// This is a convenience function for examples and tests that initializes
/// tracing with sensible defaults.
pub fn init_default() {
    use std::sync::Once;
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let filter = EnvFilter::from_default_env()
            .add_directive("aipack=info".parse().unwrap())
            .add_directive("h2=warn".parse().unwrap())
            .add_directive("hyper=warn".parse().unwrap())
            .add_directive("reqwest=warn".parse().unwrap());

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true))
            .init();
    });
}

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
