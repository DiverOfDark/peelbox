//! Pipeline context for managing dependencies

use std::path::PathBuf;
use std::sync::Arc;

use crate::fs::FileSystem;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use crate::validation::Validator;

use super::config::PipelineConfig;

/// Context that owns all long-lived pipeline dependencies
pub struct PipelineContext {
    /// LLM client for communication
    pub llm_client: Arc<dyn LLMClient>,

    /// File system abstraction
    pub file_system: Arc<dyn FileSystem>,

    /// Language registry for detection
    pub language_registry: Arc<LanguageRegistry>,

    /// Validator for build specifications
    pub validator: Arc<Validator>,

    /// Pipeline configuration
    pub config: PipelineConfig,

    /// Repository path
    pub repo_path: PathBuf,
}

impl PipelineContext {
    /// Create a new pipeline context
    pub fn new(
        llm_client: Arc<dyn LLMClient>,
        file_system: Arc<dyn FileSystem>,
        language_registry: Arc<LanguageRegistry>,
        validator: Arc<Validator>,
        config: PipelineConfig,
        repo_path: PathBuf,
    ) -> Self {
        Self {
            llm_client,
            file_system,
            language_registry,
            validator,
            config,
            repo_path,
        }
    }

    /// Create a context with default validator
    pub fn with_default_validator(
        llm_client: Arc<dyn LLMClient>,
        file_system: Arc<dyn FileSystem>,
        language_registry: Arc<LanguageRegistry>,
        config: PipelineConfig,
        repo_path: PathBuf,
    ) -> Self {
        Self::new(
            llm_client,
            file_system,
            language_registry,
            Arc::new(Validator::new()),
            config,
            repo_path,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::{MockFileSystem, RealFileSystem};
    use crate::llm::MockLLMClient;
    use std::path::PathBuf;
    use tempfile::TempDir;

    impl PipelineContext {
        /// Create a context with mocks for testing
        pub fn with_mocks() -> (Self, TempDir) {
            let temp_dir = TempDir::new().unwrap();
            let repo_path = temp_dir.path().to_path_buf();

            let llm_client = Arc::new(MockLLMClient::new());
            let file_system = Arc::new(MockFileSystem::new());
            let language_registry = Arc::new(LanguageRegistry::with_defaults());
            let validator = Arc::new(Validator::new());
            let config = PipelineConfig::default();

            let context = Self::new(
                llm_client,
                file_system,
                language_registry,
                validator,
                config,
                repo_path,
            );

            (context, temp_dir)
        }
    }

    #[test]
    fn test_context_creation() {
        let llm_client = Arc::new(MockLLMClient::new());
        let file_system = Arc::new(RealFileSystem);
        let language_registry = Arc::new(LanguageRegistry::with_defaults());
        let validator = Arc::new(Validator::new());
        let config = PipelineConfig::default();
        let repo_path = PathBuf::from("/tmp/test");

        let context = PipelineContext::new(
            llm_client,
            file_system,
            language_registry,
            validator,
            config,
            repo_path.clone(),
        );

        assert_eq!(context.repo_path, repo_path);
    }

    #[test]
    fn test_with_default_validator() {
        let llm_client = Arc::new(MockLLMClient::new());
        let file_system = Arc::new(RealFileSystem);
        let language_registry = Arc::new(LanguageRegistry::with_defaults());
        let config = PipelineConfig::default();
        let repo_path = PathBuf::from("/tmp/test");

        let context = PipelineContext::with_default_validator(
            llm_client,
            file_system,
            language_registry,
            config,
            repo_path.clone(),
        );

        assert_eq!(context.repo_path, repo_path);
    }

    #[test]
    fn test_with_mocks() {
        let (context, _temp_dir) = PipelineContext::with_mocks();
        assert!(context.repo_path.exists());
    }
}
