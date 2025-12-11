//! Detection service orchestration
//!
//! This module provides the high-level `DetectionService` that orchestrates
//! build system detection using LLM backends with tool-based analysis.
//!
//! # Architecture
//!
//! The service acts as a thin orchestration layer:
//! 1. Validates the repository path
//! 2. Delegates detection to the LLM backend
//! 3. Tracks timing metrics
//! 4. Returns validated results
//!
//! The actual repository analysis is performed iteratively by the backend
//! through tool calls (list_files, read_file, etc.).
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
//! println!("Build system: {}", result.metadata.build_system);
//! println!("Build commands: {:?}", result.build.commands);
//! println!("Confidence: {:.1}%", result.metadata.confidence * 100.0);
//! # Ok(())
//! # }
//! ```

use crate::ai::backend::BackendError;
use crate::ai::genai_backend::GenAIBackend;
use crate::config::AipackConfig;
use crate::languages::LanguageRegistry;
use crate::output::UniversalBuild;
use crate::progress::ProgressHandler;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tracing::{info, warn};

/// Errors that can occur during detection service operations
#[derive(Debug, Error)]
pub enum ServiceError {
    /// Backend error occurred during LLM communication
    #[error("Backend error: {0}")]
    BackendError(#[from] BackendError),

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
/// This service provides a simple interface for LLM-based build system detection.
/// It validates repository paths and delegates analysis to the backend, which
/// uses an iterative tool-calling approach to gather information and make decisions.
///
/// # Architecture
///
/// ```text
/// DetectionService
///   └── GenAIBackend (multi-provider with tool-based detection)
///         ├── ToolExecutor (on-demand file access)
///         ├── Iterative LLM conversation
///         └── Supports: Ollama, OpenAI, Claude, Gemini, Grok, Groq
/// ```
///
/// # Thread Safety
///
/// This service is thread-safe and can be shared across threads using `Arc`.
pub struct DetectionService {
    /// LLM backend for detection
    backend: Arc<GenAIBackend>,
    /// Language registry for build system detection (used in future phases)
    #[allow(dead_code)]
    language_registry: LanguageRegistry,
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

        let language_registry = LanguageRegistry::with_defaults();

        Ok(Self {
            backend,
            language_registry,
        })
    }

    /// Creates a new detection service with a pre-configured backend
    ///
    /// This constructor allows using a custom backend, useful for:
    /// - Automatic backend selection via `select_llm_client()`
    /// - Testing with mock backends
    /// - Custom LLM client configurations
    ///
    /// # Arguments
    ///
    /// * `backend` - Pre-configured GenAI backend
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::detection::service::DetectionService;
    /// use aipack::ai::genai_backend::{GenAIBackend, Provider};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let backend = GenAIBackend::new(Provider::Ollama, "qwen2.5-coder:7b".to_string()).await?;
    /// let service = DetectionService::with_backend(Arc::new(backend));
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_backend(backend: Arc<GenAIBackend>) -> Self {
        info!(
            "Detection service initialized with backend: {}",
            backend.name()
        );

        Self {
            backend,
            language_registry: LanguageRegistry::with_defaults(),
        }
    }

    /// Detects build system for a repository
    ///
    /// This is the main entry point for detection. It:
    /// 1. Validates the repository path exists and is a directory
    /// 2. Delegates to the LLM backend for tool-based detection
    /// 3. Tracks processing time
    /// 4. Returns the detection result
    ///
    /// The backend uses an iterative tool-calling approach where the LLM
    /// requests information about files and directory structure as needed.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository root directory
    ///
    /// # Returns
    ///
    /// A `UniversalBuild` containing the complete container build specification,
    /// including build stage, runtime stage, metadata, and confidence score.
    ///
    /// # Errors
    ///
    /// Returns `ServiceError` if:
    /// - Repository path does not exist or is not a directory
    /// - LLM backend fails or times out
    /// - Tool execution fails
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
    /// println!("Build commands: {:?}", result.build.commands);
    /// println!("Runtime command: {:?}", result.runtime.command);
    /// println!("Confidence: {:.1}%", result.metadata.confidence * 100.0);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn detect(&self, repo_path: PathBuf) -> Result<UniversalBuild, ServiceError> {
        self.detect_with_progress(repo_path, None).await
    }

    /// Detects build system for a repository with progress reporting
    ///
    /// Same as `detect()` but accepts an optional progress handler for
    /// reporting detection progress events.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository root directory
    /// * `progress` - Optional progress handler for receiving progress events
    pub async fn detect_with_progress(
        &self,
        repo_path: PathBuf,
        progress: Option<Arc<dyn ProgressHandler>>,
    ) -> Result<UniversalBuild, ServiceError> {
        let start = Instant::now();

        // Validate repository path
        self.validate_repo_path(&repo_path)?;

        info!("Starting detection for repository: {}", repo_path.display());

        // Run bootstrap scan to pre-analyze repository
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

        // Delegate to backend for tool-based detection
        let result = self
            .backend
            .detect(repo_path, bootstrap_context, progress)
            .await?;

        let elapsed = start.elapsed();

        info!(
            "Detection completed in {:.2}s: {} ({}) with {:.1}% confidence",
            elapsed.as_secs_f64(),
            result.metadata.build_system,
            result.metadata.language,
            result.metadata.confidence * 100.0
        );

        Ok(result)
    }

    /// Runs bootstrap scan to pre-analyze repository
    fn run_bootstrap_scan(
        &self,
        repo_path: &Path,
    ) -> Result<crate::bootstrap::BootstrapContext, ServiceError> {
        use crate::bootstrap::BootstrapScanner;

        let scanner =
            BootstrapScanner::with_registry(repo_path.to_path_buf(), self.language_registry.clone())
                .map_err(|e| {
                    ServiceError::DetectionFailed(format!("Bootstrap scan setup failed: {}", e))
                })?;

        scanner
            .scan()
            .map_err(|e| ServiceError::DetectionFailed(format!("Bootstrap scan failed: {}", e)))
    }

    /// Detects build system using a repository path from context
    ///
    /// This method extracts the repository path from a `RepositoryContext`
    /// and delegates to the standard `detect()` method. The context's
    /// pre-analyzed files are not used - the backend will use tools to
    /// analyze the repository on-demand.
    ///
    /// # Deprecated
    ///
    /// This method exists for backwards compatibility. Prefer using
    /// `detect(repo_path)` directly.
    ///
    /// # Arguments
    ///
    /// * `context` - Repository context (only the path is used)
    ///
    /// # Returns
    ///
    /// A `UniversalBuild` containing the complete container build specification
    ///
    /// # Errors
    ///
    /// Returns `ServiceError` if detection fails
    #[deprecated(since = "0.2.0", note = "Use detect(repo_path) instead")]
    pub async fn detect_with_context(
        &self,
        context: crate::detection::types::RepositoryContext,
    ) -> Result<UniversalBuild, ServiceError> {
        self.detect(context.repo_path).await
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
        let backend = Arc::new(
            GenAIBackend::new(Provider::Ollama, "qwen2.5-coder:7b".to_string())
                .await
                .unwrap(),
        );

        let service = DetectionService {
            backend,
            language_registry: LanguageRegistry::with_defaults(),
        };

        let result = service.validate_repo_path(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::PathNotFound(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        std::fs::write(&file_path, "content").unwrap();

        let backend = Arc::new(
            GenAIBackend::new(Provider::Ollama, "qwen2.5-coder:7b".to_string())
                .await
                .unwrap(),
        );

        let service = DetectionService {
            backend,
            language_registry: LanguageRegistry::with_defaults(),
        };

        let result = service.validate_repo_path(&file_path);
        assert!(result.is_err());
        assert!(matches!(result, Err(ServiceError::NotADirectory(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_success() {
        let temp_dir = TempDir::new().unwrap();

        let backend = Arc::new(
            GenAIBackend::new(Provider::Ollama, "qwen2.5-coder:7b".to_string())
                .await
                .unwrap(),
        );

        let service = DetectionService {
            backend,
            language_registry: LanguageRegistry::with_defaults(),
        };

        let result = service.validate_repo_path(&temp_dir.path().to_path_buf());
        assert!(result.is_ok());
    }
}
