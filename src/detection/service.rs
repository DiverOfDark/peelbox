use crate::detection::error::ServiceError;
use crate::llm::LLMClient;
use crate::output::UniversalBuild;
use crate::progress::LoggingHandler;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

pub struct DetectionService {
    client: Arc<dyn LLMClient>,
}

impl std::fmt::Debug for DetectionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectionService")
            .field("client", &self.client.name())
            .finish()
    }
}

impl DetectionService {
    pub fn new(client: Arc<dyn LLMClient>) -> Self {
        info!(
            "Detection service initialized with client: {}",
            client.name()
        );

        Self { client }
    }

    pub async fn detect(&self, repo_path: PathBuf) -> Result<Vec<UniversalBuild>, ServiceError> {
        self.detect_with_progress(repo_path, false).await
    }

    pub async fn detect_with_progress(
        &self,
        repo_path: PathBuf,
        enable_progress: bool,
    ) -> Result<Vec<UniversalBuild>, ServiceError> {
        use crate::config::DetectionMode;
        let mode = DetectionMode::from_env();
        self.detect_with_mode(repo_path, enable_progress, mode)
            .await
    }

    pub async fn detect_with_mode(
        &self,
        repo_path: PathBuf,
        enable_progress: bool,
        mode: crate::config::DetectionMode,
    ) -> Result<Vec<UniversalBuild>, ServiceError> {
        let start = Instant::now();

        self.validate_repo_path(&repo_path)?;

        info!(
            "Starting detection for repository: {} (mode: {:?})",
            repo_path.display(),
            mode
        );

        use crate::heuristics::HeuristicLogger;
        use crate::pipeline::{AnalysisContext, PipelineOrchestrator};
        use crate::stack::StackRegistry;

        let progress_handler = if enable_progress {
            Some(LoggingHandler)
        } else {
            None
        };

        let wolfi_index = crate::validation::WolfiPackageIndex::fetch().map_err(|e| {
            use crate::llm::BackendError;
            ServiceError::BackendError(BackendError::Other {
                message: format!("Failed to fetch Wolfi package index: {}", e),
            })
        })?;

        let llm_client = match mode {
            crate::config::DetectionMode::StaticOnly => None,
            _ => Some(self.client.clone()),
        };

        let mut context = AnalysisContext::new(
            &repo_path,
            Arc::new(StackRegistry::with_defaults(llm_client)),
            Arc::new(wolfi_index),
            None,
            Arc::new(HeuristicLogger::disabled()),
            mode,
        );

        let orchestrator = PipelineOrchestrator::new(progress_handler);

        let results = orchestrator
            .execute(&repo_path, &mut context)
            .await
            .map_err(|e| {
                use crate::llm::BackendError;
                ServiceError::BackendError(BackendError::Other {
                    message: e.to_string(),
                })
            })?;

        // Ensure unique service names
        Self::ensure_unique_service_names(&results)?;

        // Validate all builds with Wolfi package index
        let validator = crate::validation::Validator::with_wolfi_index(context.wolfi_index.clone());
        for build in &results {
            validator.validate(build).map_err(|e| {
                use crate::llm::BackendError;
                ServiceError::BackendError(BackendError::Other {
                    message: format!("Package validation failed: {}", e),
                })
            })?;
        }

        let elapsed = start.elapsed();

        info!(
            "Detection completed in {:.2}s: {} projects detected",
            elapsed.as_secs_f64(),
            results.len()
        );

        Ok(results)
    }

    fn validate_repo_path(&self, path: &Path) -> Result<(), ServiceError> {
        if !path.exists() {
            return Err(ServiceError::PathNotFound(path.to_path_buf()));
        }

        if !path.is_dir() {
            return Err(ServiceError::NotADirectory(path.to_path_buf()));
        }

        Ok(())
    }

    fn ensure_unique_service_names(builds: &[UniversalBuild]) -> Result<(), ServiceError> {
        use std::collections::HashSet;

        let mut seen_names: HashSet<String> = HashSet::new();
        let mut duplicates: Vec<String> = Vec::new();

        for build in builds {
            if let Some(ref name) = build.metadata.project_name {
                if !seen_names.insert(name.clone()) {
                    duplicates.push(name.clone());
                }
            }
        }

        if !duplicates.is_empty() {
            let duplicate_list = duplicates.join(", ");
            return Err(ServiceError::ConfigError(format!(
                "Duplicate service names detected: {}. Each service must have a unique name in monorepo.",
                duplicate_list
            )));
        }

        Ok(())
    }

    pub fn backend_name(&self) -> &str {
        self.client.name()
    }

    pub fn backend_model_info(&self) -> Option<String> {
        self.client.model_info()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::GenAIClient;
    use genai::adapter::AdapterKind;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_service_error_display() {
        let error = ServiceError::ConfigError("test error".to_string());
        assert_eq!(error.to_string(), "Configuration error: test error");

        let error = ServiceError::PathNotFound(PathBuf::from("/test/path"));
        assert_eq!(error.to_string(), "Repository path not found: /test/path");

        let error = ServiceError::NotADirectory(PathBuf::from("/test/file"));
        assert_eq!(
            error.to_string(),
            "Repository path is not a directory: /test/file"
        );
    }

    #[tokio::test]
    async fn test_validate_repo_path_not_exists() {
        let client = Arc::new(
            GenAIClient::new(
                AdapterKind::Ollama,
                "qwen2.5-coder:7b".to_string(),
                Duration::from_secs(30),
            )
            .await
            .unwrap(),
        ) as Arc<dyn LLMClient>;

        let service = DetectionService::new(client);

        let result = service.validate_repo_path(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::PathNotFound(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        std::fs::write(&file_path, "content").unwrap();

        let client = Arc::new(
            GenAIClient::new(
                AdapterKind::Ollama,
                "qwen2.5-coder:7b".to_string(),
                Duration::from_secs(30),
            )
            .await
            .unwrap(),
        ) as Arc<dyn LLMClient>;

        let service = DetectionService::new(client);

        let result = service.validate_repo_path(&file_path);
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::NotADirectory(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_success() {
        let temp_dir = TempDir::new().unwrap();

        let client = Arc::new(
            GenAIClient::new(
                AdapterKind::Ollama,
                "qwen2.5-coder:7b".to_string(),
                Duration::from_secs(30),
            )
            .await
            .unwrap(),
        ) as Arc<dyn LLMClient>;

        let service = DetectionService::new(client);

        let result = service.validate_repo_path(temp_dir.path());
        assert!(result.is_ok());
    }
}
