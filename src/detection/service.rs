use crate::ai::BackendError;
use crate::llm::LLMClient;
use crate::output::UniversalBuild;
use crate::pipeline::PipelineContext;
use crate::progress::ProgressHandler;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Backend error: {0}")]
    BackendError(#[from] BackendError),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Repository path not found: {0}")]
    PathNotFound(PathBuf),

    #[error("Repository path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Failed to initialize backend: {0}")]
    BackendInitError(String),

    #[error("Detection failed: {0}")]
    DetectionFailed(String),
}

impl ServiceError {
    pub fn help_message(&self) -> String {
        match self {
            ServiceError::PathNotFound(path) => {
                format!(
                    "Error: Repository path not found\nPath: {}\n\n\
                    Help: The specified path does not exist. Please check:\n\
                    - Is the path correct?\n\
                    - Does the path exist on your system?\n\
                    - Do you have permission to access it?",
                    path.display()
                )
            }
            ServiceError::NotADirectory(path) => {
                format!(
                    "Error: Repository path is not a directory\nPath: {}\n\n\
                    Help: The specified path is a file, not a directory.\n\
                    Please provide the path to the repository root directory.",
                    path.display()
                )
            }
            ServiceError::BackendInitError(msg) => {
                if msg.contains("Ollama") {
                    format!(
                        "Error: Ollama backend unavailable\n\n\
                        Help: Cannot connect to Ollama. Try:\n\
                        1. Install Ollama: https://ollama.ai/\n\
                        2. Start Ollama: ollama serve\n\
                        3. Pull a model: ollama pull qwen2.5-coder:7b\n\n\
                        Configuration:\n\
                        - AIPACK_OLLAMA_ENDPOINT (default: http://localhost:11434)\n\
                        - AIPACK_OLLAMA_MODEL (default: qwen2.5-coder:7b)\n\n\
                        Details: {}",
                        msg
                    )
                } else if msg.contains("Mistral") {
                    format!(
                        "Error: Mistral API key not configured\n\n\
                        Help: To use Mistral backend, set API key:\n\
                        export MISTRAL_API_KEY=your-key-here\n\n\
                        Get your key: https://console.mistral.ai/\n\n\
                        Details: {}",
                        msg
                    )
                } else if msg.contains("Claude") || msg.contains("OpenAI") {
                    format!(
                        "Error: Backend not yet implemented\n\n\
                        Help: This backend is not yet implemented. Try:\n\
                        - Use Ollama backend: --backend ollama\n\
                        - Use Mistral backend: --backend mistral\n\n\
                        Details: {}",
                        msg
                    )
                } else {
                    format!(
                        "Error: Failed to initialize backend\n\n\
                        Help: Try:\n\
                        - Check backend availability: aipack health\n\
                        - Use different backend: --backend <ollama|mistral>\n\n\
                        Details: {}",
                        msg
                    )
                }
            }
            ServiceError::ConfigError(msg) => {
                format!(
                    "Error: Configuration error\n\n\
                    Help: Configuration validation failed. Try:\n\
                    - Check environment variables\n\
                    - Check the documentation\n\n\
                    Details: {}",
                    msg
                )
            }
            ServiceError::BackendError(backend_err) => match backend_err {
                BackendError::TimeoutError { seconds } => {
                    format!(
                        "Error: Request timeout after {} seconds\n\n\
                            Help: The LLM request took too long. Try:\n\
                            - Increase timeout: --timeout {}\n\
                            - Check network connectivity\n\
                            - Verify backend availability: aipack health\n\
                            - Try a smaller model",
                        seconds,
                        seconds * 2
                    )
                }
                BackendError::NetworkError { message } => {
                    format!(
                        "Error: Network error\n\n\
                            Help: Cannot connect to backend. Try:\n\
                            - Check network connectivity\n\
                            - Verify backend is running: aipack health\n\
                            - Check firewall settings\n\n\
                            Details: {}",
                        message
                    )
                }
                BackendError::AuthenticationError { message } => {
                    format!(
                        "Error: Authentication failed\n\n\
                            Help: Invalid or missing credentials. Try:\n\
                            - Check API key is correct\n\
                            - Verify key has not expired\n\
                            - Check key has necessary permissions\n\n\
                            Details: {}",
                        message
                    )
                }
                BackendError::InvalidResponse { message, .. } => {
                    format!(
                        "Error: Invalid response from LLM\n\n\
                            Help: The LLM returned an unexpected response. Try:\n\
                            - Retry the operation\n\
                            - Try a different model\n\
                            - Check backend status: aipack health\n\n\
                            Details: {}",
                        message
                    )
                }
                BackendError::ParseError { message, context } => {
                    format!(
                        "Error: Failed to parse LLM response\n\n\
                            Help: The response could not be parsed. Try:\n\
                            - Retry the operation\n\
                            - Try a different model\n\
                            - Report this issue if it persists\n\n\
                            Details: {}\nContext: {}",
                        message, context
                    )
                }
                _ => {
                    format!(
                        "Error: Backend error\n\n\
                            Help: Try:\n\
                            - Check backend status: aipack health\n\
                            - Retry the operation\n\
                            - Try a different backend\n\n\
                            Details: {}",
                        backend_err
                    )
                }
            },
            ServiceError::DetectionFailed(msg) => {
                format!(
                    "Error: Detection failed\n\n\
                    Help: The detection process failed. Try:\n\
                    - Retry the operation\n\
                    - Check the repository is valid\n\
                    - Try a different backend\n\
                    - Check logs for more details\n\n\
                    Details: {}",
                    msg
                )
            }
        }
    }
}

pub struct DetectionService {
    client: Arc<dyn LLMClient>,
    context: Arc<PipelineContext>,
}

impl std::fmt::Debug for DetectionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectionService")
            .field("client", &self.client.name())
            .finish()
    }
}

impl DetectionService {
    pub fn new(client: Arc<dyn LLMClient>, context: Arc<PipelineContext>) -> Self {
        info!(
            "Detection service initialized with client: {}",
            client.name()
        );

        Self { client, context }
    }

    pub async fn detect(&self, repo_path: PathBuf) -> Result<Vec<UniversalBuild>, ServiceError> {
        self.detect_with_progress(repo_path, None).await
    }

    pub async fn detect_with_progress(
        &self,
        repo_path: PathBuf,
        progress: Option<Arc<dyn ProgressHandler>>,
    ) -> Result<Vec<UniversalBuild>, ServiceError> {
        let start = Instant::now();

        self.validate_repo_path(&repo_path)?;

        info!("Starting detection for repository: {}", repo_path.display());

        let bootstrap_context = match self.run_bootstrap_scan(&repo_path) {
            Ok(context) => {
                info!(
                    detections_found = context.detections.len(),
                    scan_time_ms = context.scan_time_ms,
                    "Bootstrap scan completed successfully"
                );
                Some(context)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "Bootstrap scan failed, continuing with normal detection"
                );
                None
            }
        };

        use crate::pipeline::AnalysisPipeline;

        let pipeline = AnalysisPipeline::new((*self.context).clone());
        let results = pipeline
            .analyze(repo_path.clone(), bootstrap_context, progress)
            .await
            .map_err(|e| {
                use crate::ai::error::BackendError;
                ServiceError::BackendError(BackendError::Other {
                    message: e.to_string(),
                })
            })?;

        let elapsed = start.elapsed();

        info!(
            "Detection completed in {:.2}s: {} projects detected",
            elapsed.as_secs_f64(),
            results.len()
        );

        Ok(results)
    }

    fn run_bootstrap_scan(
        &self,
        repo_path: &Path,
    ) -> Result<crate::bootstrap::BootstrapContext, ServiceError> {
        use crate::bootstrap::BootstrapScanner;

        let scanner = BootstrapScanner::with_registry(
            repo_path.to_path_buf(),
            self.context.language_registry.clone(),
        )
        .map_err(|e| {
            ServiceError::DetectionFailed(format!("Bootstrap scan setup failed: {}", e))
        })?;

        scanner
            .scan()
            .map_err(|e| ServiceError::DetectionFailed(format!("Bootstrap scan failed: {}", e)))
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
        use crate::pipeline::PipelineContext;

        let client = Arc::new(
            GenAIClient::new(
                AdapterKind::Ollama,
                "qwen2.5-coder:7b".to_string(),
                Duration::from_secs(30),
            )
            .await
            .unwrap(),
        ) as Arc<dyn LLMClient>;

        let (context, _temp_dir) = PipelineContext::with_mocks();
        let service = DetectionService::new(client, Arc::new(context));

        let result = service.validate_repo_path(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::PathNotFound(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_is_file() {
        use crate::pipeline::PipelineContext;

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

        let (context, _mock_temp_dir) = PipelineContext::with_mocks();
        let service = DetectionService::new(client, Arc::new(context));

        let result = service.validate_repo_path(&file_path);
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::NotADirectory(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_success() {
        use crate::pipeline::PipelineContext;

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

        let (context, _mock_temp_dir) = PipelineContext::with_mocks();
        let service = DetectionService::new(client, Arc::new(context));

        let result = service.validate_repo_path(&temp_dir.path().to_path_buf());
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_with_context() {
        use crate::pipeline::PipelineContext;

        let client = Arc::new(
            GenAIClient::new(
                AdapterKind::Ollama,
                "qwen2.5-coder:7b".to_string(),
                Duration::from_secs(30),
            )
            .await
            .unwrap(),
        ) as Arc<dyn LLMClient>;

        let (context, _temp_dir) = PipelineContext::with_mocks();
        let service = DetectionService::new(client, Arc::new(context));

        let registry = (*service.context.language_registry).clone();
        assert!(registry.get_language("rust").is_some());
    }
}
