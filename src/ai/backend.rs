//! LLM backend trait for build system detection
//!
//! This module defines the core trait that all LLM backends must implement
//! to provide build system detection capabilities.

use crate::bootstrap::BootstrapContext;
use crate::output::UniversalBuild;
use crate::progress::ProgressHandler;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

/// Errors that can occur during backend operations
pub use super::genai_backend::BackendError;

/// Backend trait for LLM-based build system detection
///
/// All LLM integrations must implement this trait to provide a consistent
/// interface for detecting build systems from repository paths.
///
/// # Tool-Based Detection
///
/// Backends use an iterative tool-calling approach where the LLM can request
/// information about the repository through tools (list_files, read_file, etc.)
/// until it has enough information to submit a final detection result.
///
/// # Bootstrap Context
///
/// An optional `BootstrapContext` can be provided to pre-populate the LLM with
/// pre-scanned repository analysis, reducing the number of tool calls needed.
///
/// # Example
///
/// ```no_run
/// use aipack::ai::backend::LLMBackend;
/// use aipack::ai::genai_backend::{GenAIBackend, Provider};
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let backend = GenAIBackend::new(
///     Provider::Ollama,
///     "qwen2.5-coder:7b".to_string(),
/// ).await?;
///
/// let result = backend.detect(PathBuf::from("/path/to/repo"), None, None).await?;
/// println!("Detected: {}", result.metadata.build_system);
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait LLMBackend: Send + Sync {
    /// Detects build system for a repository at the given path
    ///
    /// This method analyzes the repository using an iterative tool-calling
    /// approach. The LLM can request information about files, directory
    /// structure, and file contents through tools until it has enough
    /// information to make a confident detection.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository root directory
    /// * `bootstrap_context` - Optional pre-scanned repository analysis
    /// * `progress` - Optional progress handler for reporting detection progress
    ///
    /// # Returns
    ///
    /// A `UniversalBuild` containing the complete container build specification,
    /// including build stage, runtime stage, metadata, and confidence score.
    ///
    /// # Errors
    ///
    /// Returns `BackendError` if:
    /// - The LLM API request fails
    /// - The response cannot be parsed
    /// - Tool execution fails
    /// - The repository path is invalid
    async fn detect(
        &self,
        repo_path: PathBuf,
        bootstrap_context: Option<BootstrapContext>,
        progress: Option<Arc<dyn ProgressHandler>>,
    ) -> Result<UniversalBuild, BackendError>;

    /// Returns the human-readable name of this backend
    ///
    /// Used for logging and user-facing messages.
    fn name(&self) -> &str;

    /// Returns optional model information for this backend
    ///
    /// This can include the model name, version, or other identifying information.
    fn model_info(&self) -> Option<String> {
        None
    }
}
