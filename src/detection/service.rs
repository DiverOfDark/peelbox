//! Detection service orchestration
//!
//! This module provides the high-level `DetectionService` that orchestrates
//! the entire build system detection workflow. It combines:
//!
//! - Repository analysis (file scanning, context gathering)
//! - LLM backend communication
//! - Result validation and enrichment
//!
//! # Example
//!
//! ```no_run
//! use aipack::detection::service::DetectionService;
//! use aipack::AipackConfig;
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = AipackConfig::default();
//! let service = DetectionService::new(&config).await?;
//!
//! let result = service.detect(PathBuf::from("/path/to/repo")).await?;
//!
//! println!("Build system: {}", result.build_system);
//! println!("Build command: {}", result.build_command);
//! println!("Confidence: {:.1}%", result.confidence * 100.0);
//! # Ok(())
//! # }
//! ```

use crate::ai::backend::{BackendError, LLMBackend};
use crate::config::AipackConfig;
use crate::detection::analyzer::{AnalysisError, RepositoryAnalyzer};
use crate::detection::types::{DetectionResult, RepositoryContext};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, info, warn};

/// Errors that can occur during detection service operations
#[derive(Debug, Error)]
pub enum ServiceError {
    /// Backend error occurred during LLM communication
    #[error("Backend error: {0}")]
    BackendError(#[from] BackendError),

    /// Analysis error occurred during repository scanning
    #[error("Analysis error: {0}")]
    AnalysisError(#[from] AnalysisError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Repository path does not exist or is not accessible
    #[error("Repository path not found: {0}")]
    PathNotFound(PathBuf),

    /// Repository path is not a directory
    #[error("Repository path is not a directory: {0}")]
    NotADirectory(PathBuf),

    /// Backend initialization failed
    #[error("Failed to initialize backend: {0}")]
    BackendInitError(String),

    /// Detection failed with an unknown error
    #[error("Detection failed: {0}")]
    DetectionFailed(String),
}

impl ServiceError {
    /// Returns a user-friendly error message with troubleshooting hints
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
            ServiceError::AnalysisError(analysis_err) => match analysis_err {
                AnalysisError::PathNotFound(path) => {
                    format!(
                        "Error: Repository path not found\nPath: {}\n\n\
                            Help: The path does not exist. Please check:\n\
                            - Is the path correct?\n\
                            - Does the directory exist?",
                        path.display()
                    )
                }
                AnalysisError::PermissionDenied(path) => {
                    format!(
                        "Error: Permission denied\nPath: {}\n\n\
                            Help: Cannot access the directory or file. Try:\n\
                            - Check file permissions\n\
                            - Ensure you have read access\n\
                            - Try running with appropriate permissions",
                        path
                    )
                }
                AnalysisError::TooLarge(limit) => {
                    format!(
                        "Error: Repository too large\n\n\
                            Help: Repository exceeded file limit of {} entries.\n\
                            This usually indicates a very large repository.\n\n\
                            Try:\n\
                            - Analyze a subdirectory instead\n\
                            - Clean up build artifacts (target/, node_modules/, etc.)\n\
                            - Use .gitignore patterns to exclude large directories",
                        limit
                    )
                }
                _ => {
                    format!(
                        "Error: Repository analysis failed\n\n\
                            Help: Failed to analyze repository. Try:\n\
                            - Check the repository is valid\n\
                            - Ensure you have read permissions\n\
                            - Try a different directory\n\n\
                            Details: {}",
                        analysis_err
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

/// High-level detection service that orchestrates the detection workflow
///
/// This service combines repository analysis with LLM-based detection to
/// identify build systems and generate appropriate commands. It provides
/// a simple, high-level API for detection operations.
///
/// # Architecture
///
/// ```text
/// DetectionService
///   ├── RepositoryAnalyzer  (scans files, builds context)
///   └── LLMBackend          (AI-powered detection)
///         └── OllamaClient (or other backends)
/// ```
///
/// # Thread Safety
///
/// This service is thread-safe and can be shared across threads using `Arc`.
pub struct DetectionService {
    /// LLM backend for detection
    backend: Arc<dyn LLMBackend>,
}

impl std::fmt::Debug for DetectionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectionService")
            .field("backend", &self.backend.name())
            .finish()
    }
}

impl DetectionService {
    /// Creates a new detection service from configuration
    ///
    /// This is the primary factory method that:
    /// 1. Validates the configuration
    /// 2. Creates the appropriate LLM backend
    /// 3. Initializes the repository analyzer
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration
    ///
    /// # Returns
    ///
    /// A configured `DetectionService` ready for use
    ///
    /// # Errors
    ///
    /// Returns `ServiceError` if:
    /// - Configuration is invalid
    /// - Backend cannot be initialized
    /// - Required credentials are missing
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::detection::service::DetectionService;
    /// use aipack::AipackConfig;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AipackConfig::default();
    /// let service = DetectionService::new(&config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(config: &AipackConfig) -> Result<Self, ServiceError> {
        info!("Initializing detection service");

        // Create backend directly from configuration
        let backend = config
            .create_backend()
            .await
            .map_err(|e| ServiceError::ConfigError(e.to_string()))?;

        info!(
            "Detection service initialized with backend: {}",
            backend.name()
        );

        Ok(Self { backend })
    }

    /// Detects build system for a repository
    ///
    /// This is the main entry point for detection. It:
    /// 1. Validates the repository path
    /// 2. Analyzes the repository to gather context
    /// 3. Calls the LLM backend to detect the build system
    /// 4. Returns a validated, enriched result
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository root
    ///
    /// # Returns
    ///
    /// A `DetectionResult` containing build system information and commands
    ///
    /// # Errors
    ///
    /// Returns `ServiceError` if:
    /// - Repository path does not exist or is not a directory
    /// - Repository analysis fails
    /// - LLM backend fails or times out
    /// - Response parsing fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::detection::service::DetectionService;
    /// use aipack::AipackConfig;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AipackConfig::default();
    /// let service = DetectionService::new(&config).await?;
    ///
    /// let result = service.detect(PathBuf::from("/path/to/my-project")).await?;
    ///
    /// println!("Build: {}", result.build_command);
    /// println!("Test: {}", result.test_command);
    /// println!("Deploy: {}", result.deploy_command);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn detect(&self, repo_path: PathBuf) -> Result<DetectionResult, ServiceError> {
        let start = Instant::now();

        // Validate repository path
        self.validate_repo_path(&repo_path)?;

        info!("Starting detection for repository: {}", repo_path.display());

        // Analyze repository
        debug!("Analyzing repository structure and contents");
        let analyzer = RepositoryAnalyzer::new(repo_path.clone());
        let context = analyzer.analyze().await?;

        info!(
            "Repository analysis complete: {} key files detected",
            context.key_file_count()
        );

        // Perform detection with backend
        debug!("Calling LLM backend for detection");
        let mut result = self.backend.detect(context).await?;

        // Set processing time
        result.processing_time_ms = start.elapsed().as_millis() as u64;

        info!(
            "Detection completed in {:.2}s: {} ({}) with {:.1}% confidence",
            start.elapsed().as_secs_f64(),
            result.build_system,
            result.language,
            result.confidence * 100.0
        );

        if result.has_warnings() {
            warn!("Detection warnings: {:?}", result.warnings);
        }

        Ok(result)
    }

    /// Detects build system using a custom repository context
    ///
    /// This method allows you to provide a pre-built `RepositoryContext` instead
    /// of analyzing a repository from disk. Useful for testing or when you already
    /// have the context from another source.
    ///
    /// # Arguments
    ///
    /// * `context` - Pre-built repository context
    ///
    /// # Returns
    ///
    /// A `DetectionResult` containing build system information and commands
    ///
    /// # Errors
    ///
    /// Returns `ServiceError` if LLM backend fails or response parsing fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::detection::service::DetectionService;
    /// use aipack::detection::types::RepositoryContext;
    /// use aipack::AipackConfig;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AipackConfig::default();
    /// let service = DetectionService::new(&config).await?;
    ///
    /// let context = RepositoryContext::minimal(
    ///     PathBuf::from("/test/repo"),
    ///     "repo/\n├── package.json\n└── src/".to_string(),
    /// ).with_key_file(
    ///     "package.json".to_string(),
    ///     r#"{"name": "test", "scripts": {"build": "tsc"}}"#.to_string(),
    /// );
    ///
    /// let result = service.detect_with_context(context).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn detect_with_context(
        &self,
        context: RepositoryContext,
    ) -> Result<DetectionResult, ServiceError> {
        let start = Instant::now();

        info!(
            "Starting detection with custom context for: {}",
            context.repo_path.display()
        );

        // Perform detection with backend
        let mut result = self.backend.detect(context).await?;

        // Set processing time
        result.processing_time_ms = start.elapsed().as_millis() as u64;

        info!(
            "Detection completed in {:.2}s: {} ({})",
            start.elapsed().as_secs_f64(),
            result.build_system,
            result.language
        );

        Ok(result)
    }

    /// Validates that a repository path exists and is a directory
    fn validate_repo_path(&self, path: &Path) -> Result<(), ServiceError> {
        if !path.exists() {
            return Err(ServiceError::PathNotFound(path.to_path_buf()));
        }

        if !path.is_dir() {
            return Err(ServiceError::NotADirectory(path.to_path_buf()));
        }

        Ok(())
    }

    /// Returns the name of the backend being used
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::detection::service::DetectionService;
    /// use aipack::AipackConfig;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AipackConfig::default();
    /// let service = DetectionService::new(&config).await?;
    ///
    /// println!("Using backend: {}", service.backend_name());
    /// # Ok(())
    /// # }
    /// ```
    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    /// Returns model information for the backend
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::detection::service::DetectionService;
    /// use aipack::AipackConfig;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AipackConfig::default();
    /// let service = DetectionService::new(&config).await?;
    ///
    /// if let Some(info) = service.backend_model_info() {
    ///     println!("Model: {}", info);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn backend_model_info(&self) -> Option<String> {
        self.backend.model_info()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::genai_backend::{GenAIBackend, Provider};
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
        let backend = Arc::new(GenAIBackend::new(
            Provider::Ollama,
            "qwen2.5-coder:7b".to_string(),
        ).await.unwrap()) as Arc<dyn LLMBackend>;

        let service = DetectionService { backend };

        let result = service.validate_repo_path(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::PathNotFound(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        std::fs::write(&file_path, "content").unwrap();

        let backend = Arc::new(GenAIBackend::new(
            Provider::Ollama,
            "qwen2.5-coder:7b".to_string(),
        ).await.unwrap()) as Arc<dyn LLMBackend>;

        let service = DetectionService { backend };

        let result = service.validate_repo_path(&file_path);
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::NotADirectory(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_success() {
        let temp_dir = TempDir::new().unwrap();

        let backend = Arc::new(GenAIBackend::new(
            Provider::Ollama,
            "qwen2.5-coder:7b".to_string(),
        ).await.unwrap()) as Arc<dyn LLMBackend>;

        let service = DetectionService { backend };

        let result = service.validate_repo_path(&temp_dir.path().to_path_buf());
        assert!(result.is_ok());
    }

    // Integration tests with actual Ollama server are in tests/ollama_integration.rs
}
