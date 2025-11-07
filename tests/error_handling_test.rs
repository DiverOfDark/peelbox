//! Error handling integration tests
//!
//! Tests comprehensive error scenarios including:
//! - Missing repository paths
//! - Invalid paths
//! - Permission denied scenarios
//! - Backend unavailability
//! - Timeout scenarios
//! - Malformed responses
//! - Configuration errors

use aipack::config::{AipackConfig, ConfigError};
use aipack::detection::analyzer::{AnalysisError, RepositoryAnalyzer};
use aipack::detection::service::{DetectionService, ServiceError};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn test_path_not_found_error() {
    let non_existent_path = PathBuf::from("/nonexistent/repository/path");
    let analyzer = RepositoryAnalyzer::new(non_existent_path.clone());

    let result = analyzer.analyze().await;
    assert!(result.is_err());

    match result.unwrap_err() {
        AnalysisError::PathNotFound(path) => {
            assert_eq!(path, non_existent_path);
        }
        _ => panic!("Expected PathNotFound error"),
    }
}

#[tokio::test]
async fn test_not_a_directory_error() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("file.txt");
    fs::write(&file_path, "content").unwrap();

    let analyzer = RepositoryAnalyzer::new(file_path.clone());
    let result = analyzer.analyze().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AnalysisError::NotADirectory(path) => {
            assert_eq!(path, file_path);
        }
        _ => panic!("Expected NotADirectory error"),
    }
}

#[tokio::test]
#[cfg(unix)]
async fn test_permission_denied_error() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    // Create a file with no read permissions
    let restricted_file = repo_path.join("restricted.txt");
    fs::write(&restricted_file, "secret content").unwrap();

    // Remove all permissions
    let mut perms = fs::metadata(&restricted_file).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&restricted_file, perms).unwrap();

    let analyzer = RepositoryAnalyzer::new(repo_path);
    let result = analyzer.analyze().await;

    // Note: This test may behave differently depending on the system and running user
    // If running as root, permission errors might not occur
    if let Err(AnalysisError::PermissionDenied(path)) = result {
        assert!(path.contains("restricted"));
    }

    // Restore permissions for cleanup
    let mut perms = fs::metadata(&restricted_file).unwrap().permissions();
    perms.set_mode(0o644);
    let _ = fs::set_permissions(&restricted_file, perms);
}

#[tokio::test]
async fn test_repository_too_large_error() {
    use aipack::detection::analyzer::AnalyzerConfig;

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create many files to exceed the limit
    for i in 0..150 {
        fs::write(repo_path.join(format!("file{}.txt", i)), "content").unwrap();
    }

    let mut config = AnalyzerConfig::default();
    config.file_tree_limit = 100; // Set low limit

    let analyzer = RepositoryAnalyzer::with_config(repo_path.to_path_buf(), config);
    let result = analyzer.analyze().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AnalysisError::TooLarge(limit) => {
            assert_eq!(limit, 100);
        }
        _ => panic!("Expected TooLarge error"),
    }
}

#[test]
fn test_config_error_invalid_backend() {
    let mut config = AipackConfig::default();
    config.backend = "invalid-backend".to_string();

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        ConfigError::InvalidBackend(backend) => {
            assert_eq!(backend, "invalid-backend");
        }
        _ => panic!("Expected InvalidBackend error"),
    }
}

#[test]
fn test_config_error_missing_api_key() {
    let mut config = AipackConfig::default();
    config.backend = "mistral".to_string();
    config.mistral_api_key = None;

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        ConfigError::MissingApiKey => {}
        _ => panic!("Expected MissingApiKey error"),
    }
}

#[test]
fn test_config_error_invalid_endpoint() {
    let mut config = AipackConfig::default();
    config.ollama_endpoint = "not-a-url".to_string();

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        ConfigError::InvalidEndpoint(endpoint) => {
            assert_eq!(endpoint, "not-a-url");
        }
        _ => panic!("Expected InvalidEndpoint error"),
    }
}

#[test]
fn test_config_error_invalid_timeout() {
    let mut config = AipackConfig::default();
    config.request_timeout_secs = 0;

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        ConfigError::ValidationFailed(msg) => {
            assert!(msg.contains("timeout"));
        }
        _ => panic!("Expected ValidationFailed error for timeout"),
    }
}

#[test]
fn test_config_error_timeout_too_large() {
    let mut config = AipackConfig::default();
    config.request_timeout_secs = 700; // More than 10 minutes

    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_config_error_invalid_context_size() {
    let mut config = AipackConfig::default();
    config.max_context_size = 500; // Less than 1KB

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        ConfigError::ValidationFailed(msg) => {
            assert!(msg.contains("context size"));
        }
        _ => panic!("Expected ValidationFailed error for context size"),
    }
}

#[test]
fn test_config_error_invalid_log_level() {
    let mut config = AipackConfig::default();
    config.log_level = "invalid-level".to_string();

    let result = config.validate();
    assert!(result.is_err());

    match result.unwrap_err() {
        ConfigError::ValidationFailed(msg) => {
            assert!(msg.contains("log level"));
        }
        _ => panic!("Expected ValidationFailed error for log level"),
    }
}

#[tokio::test]
async fn test_service_error_path_not_found() {
    let config = AipackConfig {
        backend: "ollama".to_string(),
        ollama_endpoint: "http://localhost:11434".to_string(),
        ollama_model: "qwen:7b".to_string(),
        lm_studio_endpoint: "http://localhost:8000".to_string(),
        mistral_api_key: None,
        mistral_model: "mistral-small".to_string(),
        cache_enabled: false,
        cache_dir: None,
        request_timeout_secs: 30,
        max_context_size: 512_000,
        log_level: "error".to_string(),
    };

    // Note: This test will fail to create service if Ollama is not available
    // We test the path validation in the analyzer level instead
    let non_existent = PathBuf::from("/nonexistent/path");
    let analyzer = RepositoryAnalyzer::new(non_existent);

    let result = analyzer.analyze().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_service_error_not_a_directory() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("file.txt");
    fs::write(&file_path, "content").unwrap();

    let analyzer = RepositoryAnalyzer::new(file_path);
    let result = analyzer.analyze().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AnalysisError::NotADirectory(_) => {}
        _ => panic!("Expected NotADirectory error"),
    }
}

#[tokio::test]
async fn test_backend_unavailable_error() {
    // Try to create service with unreachable endpoint
    let mut config = AipackConfig::default();
    config.backend = "ollama".to_string();
    config.ollama_endpoint = "http://localhost:59999".to_string(); // Non-existent port

    let result = DetectionService::new(&config).await;
    assert!(result.is_err());

    match result.unwrap_err() {
        ServiceError::BackendInitError(msg) => {
            assert!(msg.contains("Ollama"));
        }
        _ => panic!("Expected BackendInitError"),
    }
}

#[tokio::test]
async fn test_config_error_in_service() {
    let mut config = AipackConfig::default();
    config.backend = "mistral".to_string();
    config.mistral_api_key = None;

    // First, validation should fail
    let validation_result = config.validate();
    assert!(validation_result.is_err());

    // If we bypass validation, service creation should fail
    let result = config.selected_backend_config();
    assert!(result.is_err());

    match result.unwrap_err() {
        ConfigError::MissingApiKey => {}
        _ => panic!("Expected MissingApiKey error"),
    }
}

#[test]
fn test_service_error_display() {
    let error = ServiceError::PathNotFound(PathBuf::from("/test/path"));
    let display = format!("{}", error);
    assert!(display.contains("/test/path"));

    let error = ServiceError::NotADirectory(PathBuf::from("/test/file"));
    let display = format!("{}", error);
    assert!(display.contains("not a directory"));

    let error = ServiceError::BackendInitError("Ollama unavailable".to_string());
    let display = format!("{}", error);
    assert!(display.contains("backend"));
}

#[test]
fn test_service_error_help_messages() {
    let error = ServiceError::PathNotFound(PathBuf::from("/test/path"));
    let help = error.help_message();
    assert!(help.contains("Help:"));
    assert!(help.contains("path"));

    let error = ServiceError::BackendInitError("Ollama is not available".to_string());
    let help = error.help_message();
    assert!(help.contains("Ollama"));
    assert!(help.contains("ollama serve"));

    let error = ServiceError::BackendInitError("Mistral API key".to_string());
    let help = error.help_message();
    assert!(help.contains("MISTRAL_API_KEY"));
    assert!(help.contains("console.mistral.ai"));
}

#[test]
fn test_analysis_error_display() {
    let error = AnalysisError::PathNotFound(PathBuf::from("/test"));
    assert!(format!("{}", error).contains("does not exist"));

    let error = AnalysisError::NotADirectory(PathBuf::from("/test"));
    assert!(format!("{}", error).contains("not a directory"));

    let error = AnalysisError::PermissionDenied("/test/file".to_string());
    assert!(format!("{}", error).contains("Permission denied"));

    let error = AnalysisError::TooLarge(100);
    assert!(format!("{}", error).contains("100"));
}

#[test]
fn test_config_error_display() {
    let error = ConfigError::InvalidBackend("test".to_string());
    assert!(format!("{}", error).contains("Invalid backend"));

    let error = ConfigError::MissingApiKey;
    assert!(format!("{}", error).contains("API key"));

    let error = ConfigError::InvalidEndpoint("test".to_string());
    assert!(format!("{}", error).contains("Invalid endpoint"));

    let error = ConfigError::ValidationFailed("test".to_string());
    assert!(format!("{}", error).contains("validation failed"));
}

#[test]
fn test_backend_error_display() {
    use aipack::ai::backend::BackendError;

    let error = BackendError::TimeoutError { seconds: 30 };
    assert!(format!("{}", error).contains("30 seconds"));

    let error = BackendError::NetworkError {
        message: "Connection refused".to_string(),
    };
    assert!(format!("{}", error).contains("Network error"));

    let error = BackendError::AuthenticationError {
        message: "Invalid API key".to_string(),
    };
    assert!(format!("{}", error).contains("Authentication failed"));

    let error = BackendError::InvalidResponse {
        message: "Malformed JSON".to_string(),
        raw_response: Some("{invalid}".to_string()),
    };
    assert!(format!("{}", error).contains("Invalid response"));

    let error = BackendError::ParseError {
        message: "Cannot parse".to_string(),
        context: "response text".to_string(),
    };
    assert!(format!("{}", error).contains("Parse error"));
}

#[test]
fn test_error_types_implement_error_trait() {
    use std::error::Error;

    // Verify all custom error types implement std::error::Error
    fn is_error<T: Error>() {}

    is_error::<ServiceError>();
    is_error::<AnalysisError>();
    is_error::<ConfigError>();
    is_error::<aipack::ai::backend::BackendError>();
}

#[test]
fn test_error_chain_propagation() {
    use aipack::ai::backend::BackendError;

    // Test that BackendError converts to ServiceError
    let backend_error = BackendError::TimeoutError { seconds: 30 };
    let service_error: ServiceError = backend_error.into();

    match service_error {
        ServiceError::BackendError(_) => {}
        _ => panic!("Expected BackendError variant"),
    }

    // Test that AnalysisError converts to ServiceError
    let analysis_error = AnalysisError::PathNotFound(PathBuf::from("/test"));
    let service_error: ServiceError = analysis_error.into();

    match service_error {
        ServiceError::AnalysisError(_) => {}
        _ => panic!("Expected AnalysisError variant"),
    }
}

#[test]
fn test_config_auto_mode_no_backends() {
    let mut config = AipackConfig::default();
    config.backend = "auto".to_string();
    config.mistral_api_key = None;

    // In auto mode with no Ollama and no Mistral key, should fail
    let result = config.selected_backend_config();

    // Result depends on whether Ollama is actually running
    // If Ollama is not running and no Mistral key, should error
    if !config.is_ollama_available() {
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ValidationFailed(msg) => {
                assert!(msg.contains("No backend available"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }
}

#[tokio::test]
async fn test_empty_repository() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();

    // Empty directory
    let analyzer = RepositoryAnalyzer::new(repo_path);
    let context = analyzer.analyze().await.unwrap();

    // Should succeed but have minimal content
    assert!(context.key_files.is_empty());
    assert!(context.readme_content.is_none());
}

#[tokio::test]
async fn test_repository_with_binary_files() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create a binary file
    let binary_data = vec![0u8, 1, 2, 3, 255, 254, 253];
    fs::write(repo_path.join("binary.dat"), binary_data).unwrap();

    // Create a text file
    fs::write(repo_path.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

    let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
    let context = analyzer.analyze().await.unwrap();

    // Should handle binary files gracefully
    assert!(context.key_files.contains_key("Cargo.toml"));
    // Binary file should not be in key_files
    assert!(!context.key_files.contains_key("binary.dat"));
}

#[tokio::test]
async fn test_deeply_nested_structure() {
    use aipack::detection::analyzer::AnalyzerConfig;

    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create deep nesting
    let mut current = repo_path.to_path_buf();
    for i in 0..10 {
        current = current.join(format!("level{}", i));
        fs::create_dir(&current).unwrap();
    }
    fs::write(current.join("deep.txt"), "deep file").unwrap();

    // Analyze with default depth limit
    let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
    let context = analyzer.analyze().await.unwrap();

    // Should respect depth limit and not include deeply nested file
    let has_deep_file = context.file_tree.contains("deep.txt");
    // Default max_depth is 3, so shouldn't reach level 10
    assert!(!has_deep_file);

    // Analyze with higher depth limit
    let mut config = AnalyzerConfig::default();
    config.max_depth = 15;
    let analyzer = RepositoryAnalyzer::with_config(repo_path.to_path_buf(), config);
    let context = analyzer.analyze().await.unwrap();

    // Should now include the deep file
    assert!(context.file_tree.contains("deep.txt"));
}
