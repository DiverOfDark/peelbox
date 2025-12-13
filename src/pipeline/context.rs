use std::sync::Arc;

use crate::fs::FileSystem;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use crate::validation::Validator;

use super::config::PipelineConfig;

#[derive(Clone)]
pub struct PipelineContext {
    pub llm_client: Arc<dyn LLMClient>,
    pub file_system: Arc<dyn FileSystem>,
    pub language_registry: Arc<LanguageRegistry>,
    pub validator: Arc<Validator>,
    pub config: PipelineConfig,
}

impl PipelineContext {
    pub fn new(
        llm_client: Arc<dyn LLMClient>,
        file_system: Arc<dyn FileSystem>,
        language_registry: Arc<LanguageRegistry>,
        validator: Arc<Validator>,
        config: PipelineConfig,
    ) -> Self {
        Self {
            llm_client,
            file_system,
            language_registry,
            validator,
            config,
        }
    }

    pub fn with_default_validator(
        llm_client: Arc<dyn LLMClient>,
        file_system: Arc<dyn FileSystem>,
        language_registry: Arc<LanguageRegistry>,
        config: PipelineConfig,
    ) -> Self {
        Self::new(
            llm_client,
            file_system,
            language_registry,
            Arc::new(Validator::new()),
            config,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::{MockFileSystem, RealFileSystem};
    use crate::llm::MockLLMClient;
    use tempfile::TempDir;

    impl PipelineContext {
        pub fn with_mocks() -> (Self, TempDir) {
            let temp_dir = TempDir::new().unwrap();

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

        let context = PipelineContext::new(
            llm_client,
            file_system,
            language_registry,
            validator,
            config,
        );

        assert_eq!(
            context.config.max_iterations,
            PipelineConfig::default().max_iterations
        );
    }

    #[test]
    fn test_with_default_validator() {
        let llm_client = Arc::new(MockLLMClient::new());
        let file_system = Arc::new(RealFileSystem);
        let language_registry = Arc::new(LanguageRegistry::with_defaults());
        let config = PipelineConfig::default();

        let context = PipelineContext::with_default_validator(
            llm_client,
            file_system,
            language_registry,
            config,
        );

        // Just verify it was created successfully
        assert_eq!(
            context.config.max_iterations,
            PipelineConfig::default().max_iterations
        );
    }

    #[test]
    fn test_with_mocks() {
        let (context, _temp_dir) = PipelineContext::with_mocks();
        // Just verify the context was created with mocks
        assert!(context.language_registry.get_language("rust").is_some());
    }
}
