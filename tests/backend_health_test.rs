//! Backend health check integration tests
//!
//! Tests backend availability checking, configuration validation,
//! and health status reporting.

use aipack::ai::ollama::OllamaClient;
use aipack::config::AipackConfig;
use aipack::detection::service::DetectionService;
use std::time::Duration;

#[tokio::test]
async fn test_ollama_health_check_unavailable() {
    // Test with non-existent endpoint
    let client = OllamaClient::with_timeout(
        "http://localhost:59999".to_string(),
        "qwen:7b".to_string(),
        Duration::from_millis(500),
    );

    let result = client.health_check().await;

    // Should return Ok(false) for unreachable endpoint
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn test_ollama_client_timeout() {
    // Test with very short timeout
    let client = OllamaClient::with_timeout(
        "http://localhost:11434".to_string(),
        "qwen:7b".to_string(),
        Duration::from_millis(1), // Very short timeout
    );

    let result = client.health_check().await;

    // Should handle timeout gracefully (Ok(false) for timeout)
    assert!(result.is_ok());
}

#[test]
fn test_config_ollama_availability_check() {
    let config = AipackConfig {
        backend: "ollama".to_string(),
        ollama_endpoint: "http://localhost:59999".to_string(), // Non-existent port
        ollama_model: "qwen:7b".to_string(),
        mistral_api_key: None,
        mistral_model: "mistral-small".to_string(),
        cache_enabled: false,
        cache_dir: None,
        request_timeout_secs: 2,
        max_context_size: 512_000,
        log_level: "error".to_string(),
    };

    // Should return false for unreachable endpoint
    let is_available = config.is_ollama_available();
    assert!(!is_available);
}

#[test]
fn test_config_mistral_key_check() {
    let mut config = AipackConfig::default();

    // Without key
    assert!(!config.has_mistral_key());

    // With key
    config.mistral_api_key = Some("test-key".to_string());
    assert!(config.has_mistral_key());
}

#[tokio::test]
async fn test_service_creation_with_invalid_backend() {
    let mut config = AipackConfig::default();
    config.backend = "ollama".to_string();
    config.ollama_endpoint = "http://localhost:59999".to_string();

    let result = DetectionService::new(&config).await;

    // Should fail to create service
    assert!(result.is_err());
}

#[test]
fn test_config_backend_selection_ollama() {
    let mut config = AipackConfig::default();
    config.backend = "ollama".to_string();

    let backend_config = config.selected_backend_config();

    assert!(backend_config.is_ok());
    let backend = backend_config.unwrap();

    // Verify it's a Local (Ollama) backend
    use aipack::ai::backend::BackendConfig;
    match backend {
        BackendConfig::Local {
            endpoint, model, ..
        } => {
            assert_eq!(endpoint, config.ollama_endpoint);
            assert_eq!(model, config.ollama_model);
        }
        _ => panic!("Expected Local backend config"),
    }
}

#[test]
fn test_config_backend_selection_mistral() {
    let mut config = AipackConfig::default();
    config.backend = "mistral".to_string();
    config.mistral_api_key = Some("test-key".to_string());

    let backend_config = config.selected_backend_config();

    assert!(backend_config.is_ok());
    let backend = backend_config.unwrap();

    // Verify it's an OpenAI (Mistral-compatible) backend
    use aipack::ai::backend::BackendConfig;
    match backend {
        BackendConfig::OpenAI { api_key, model, .. } => {
            assert_eq!(api_key, "test-key");
            assert_eq!(model, config.mistral_model);
        }
        _ => panic!("Expected OpenAI backend config for Mistral"),
    }
}

#[test]
fn test_config_backend_selection_mistral_no_key() {
    let mut config = AipackConfig::default();
    config.backend = "mistral".to_string();
    config.mistral_api_key = None;

    let backend_config = config.selected_backend_config();

    // Should fail without API key
    assert!(backend_config.is_err());
}

#[test]
fn test_config_backend_selection_auto_no_backends() {
    let mut config = AipackConfig::default();
    config.backend = "auto".to_string();
    config.ollama_endpoint = "http://localhost:59999".to_string(); // Unreachable
    config.mistral_api_key = None; // No key

    let backend_config = config.selected_backend_config();

    // Should fail if neither backend is available
    // Note: This might succeed if Ollama is actually running on localhost:11434
    if !config.is_ollama_available() {
        assert!(backend_config.is_err());
    }
}

#[test]
fn test_ollama_client_name_and_info() {
    let client = OllamaClient::new("http://localhost:11434".to_string(), "qwen:7b".to_string());

    use aipack::ai::backend::LLMBackend;

    assert_eq!(client.name(), "ollama");
    assert!(client.model_info().is_some());
    assert!(client.model_info().unwrap().contains("qwen:7b"));
    assert!(client.model_info().unwrap().contains("localhost:11434"));
}

#[test]
fn test_ollama_client_custom_timeout() {
    let client = OllamaClient::with_timeout(
        "http://localhost:11434".to_string(),
        "qwen:7b".to_string(),
        Duration::from_secs(120),
    );

    assert_eq!(format!("{:?}", client).contains("120"), true);
}

#[test]
fn test_backend_config_timeout_defaults() {
    use aipack::ai::backend::BackendConfig;

    let config = BackendConfig::Local {
        model: "qwen:7b".to_string(),
        endpoint: "http://localhost:11434".to_string(),
        timeout_seconds: None,
        max_tokens: None,
    };
    assert_eq!(config.timeout_seconds(), 60); // Default for Local

    let config = BackendConfig::OpenAI {
        api_key: "test".to_string(),
        model: "gpt-4".to_string(),
        organization_id: None,
        api_endpoint: None,
        timeout_seconds: None,
        max_tokens: None,
    };
    assert_eq!(config.timeout_seconds(), 30); // Default for OpenAI

    let config = BackendConfig::Claude {
        api_key: "test".to_string(),
        model: "claude-3".to_string(),
        api_endpoint: None,
        timeout_seconds: Some(45),
        max_tokens: None,
    };
    assert_eq!(config.timeout_seconds(), 45); // Custom timeout
}

#[test]
fn test_backend_config_model_name() {
    use aipack::ai::backend::BackendConfig;

    let config = BackendConfig::Local {
        model: "qwen:14b".to_string(),
        endpoint: "http://localhost:11434".to_string(),
        timeout_seconds: None,
        max_tokens: None,
    };
    assert_eq!(config.model_name(), "qwen:14b");

    let config = BackendConfig::OpenAI {
        api_key: "test".to_string(),
        model: "gpt-4-turbo".to_string(),
        organization_id: None,
        api_endpoint: None,
        timeout_seconds: None,
        max_tokens: None,
    };
    assert_eq!(config.model_name(), "gpt-4-turbo");
}

#[test]
fn test_config_validation_all_fields() {
    let config = AipackConfig {
        backend: "ollama".to_string(),
        ollama_endpoint: "http://localhost:11434".to_string(),
        ollama_model: "qwen:7b".to_string(),
        mistral_api_key: None,
        mistral_model: "mistral-small".to_string(),
        cache_enabled: true,
        cache_dir: Some(std::path::PathBuf::from("/tmp/cache")),
        request_timeout_secs: 30,
        max_context_size: 512_000,
        log_level: "info".to_string(),
    };

    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_edge_cases() {
    // Test minimum valid timeout
    let mut config = AipackConfig::default();
    config.request_timeout_secs = 1;
    assert!(config.validate().is_ok());

    // Test maximum valid timeout
    config.request_timeout_secs = 600;
    assert!(config.validate().is_ok());

    // Test minimum valid context size
    config.max_context_size = 1024;
    assert!(config.validate().is_ok());

    // Test maximum valid context size
    config.max_context_size = 10_485_760;
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_all_log_levels() {
    let mut config = AipackConfig::default();

    for level in &["trace", "debug", "info", "warn", "error"] {
        config.log_level = level.to_string();
        assert!(
            config.validate().is_ok(),
            "Log level {} should be valid",
            level
        );
    }

    config.log_level = "invalid".to_string();
    assert!(config.validate().is_err());
}

#[test]
fn test_config_cache_path_generation() {
    let config = AipackConfig {
        backend: "ollama".to_string(),
        ollama_endpoint: "http://localhost:11434".to_string(),
        ollama_model: "qwen:7b".to_string(),
        mistral_api_key: None,
        mistral_model: "mistral-small".to_string(),
        cache_enabled: true,
        cache_dir: Some(std::path::PathBuf::from("/tmp/cache")),
        request_timeout_secs: 30,
        max_context_size: 512_000,
        log_level: "info".to_string(),
    };

    let path = config.cache_path("my-repo");
    assert!(path.to_string_lossy().contains("my-repo.json"));

    // Test special character sanitization
    let path = config.cache_path("user/repo:branch");
    assert!(path.to_string_lossy().contains("user_repo_branch.json"));
}

#[tokio::test]
async fn test_health_check_with_multiple_endpoints() {
    // Test multiple endpoints in sequence
    let endpoints = vec![
        "http://localhost:59991".to_string(),
        "http://localhost:59992".to_string(),
        "http://localhost:59993".to_string(),
    ];

    for endpoint in endpoints {
        let client = OllamaClient::with_timeout(
            endpoint.clone(),
            "qwen:7b".to_string(),
            Duration::from_millis(100),
        );

        let result = client.health_check().await;
        assert!(result.is_ok());
        assert!(
            !result.unwrap(),
            "Endpoint {} should be unavailable",
            endpoint
        );
    }
}

#[test]
fn test_config_display_formatting() {
    let config = AipackConfig {
        backend: "ollama".to_string(),
        ollama_endpoint: "http://localhost:11434".to_string(),
        ollama_model: "qwen:7b".to_string(),
        mistral_api_key: Some("secret-key".to_string()),
        mistral_model: "mistral-small".to_string(),
        cache_enabled: true,
        cache_dir: Some(std::path::PathBuf::from("/tmp/cache")),
        request_timeout_secs: 30,
        max_context_size: 512_000,
        log_level: "info".to_string(),
    };

    let display = format!("{}", config);

    // Verify key fields are displayed
    assert!(display.contains("ollama"));
    assert!(display.contains("http://localhost:11434"));
    assert!(display.contains("qwen:7b"));
    assert!(display.contains("Set")); // API key should show as "Set", not actual value
    assert!(display.contains("30s"));
    assert!(display.contains("info"));
}

#[test]
fn test_backend_error_types() {
    use aipack::ai::backend::BackendError;

    // Test TimeoutError
    let error = BackendError::TimeoutError { seconds: 30 };
    let display = format!("{}", error);
    assert!(display.contains("30 seconds"));

    // Test NetworkError
    let error = BackendError::NetworkError {
        message: "Connection refused".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("Network error"));

    // Test AuthenticationError
    let error = BackendError::AuthenticationError {
        message: "Invalid key".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("Authentication"));

    // Test RateLimitError
    let error = BackendError::RateLimitError {
        retry_after: Some(60),
    };
    let display = format!("{}", error);
    assert!(display.contains("Rate limit"));
    assert!(display.contains("60"));

    let error = BackendError::RateLimitError { retry_after: None };
    let display = format!("{}", error);
    assert!(display.contains("Rate limit"));

    // Test InvalidResponse
    let error = BackendError::InvalidResponse {
        message: "Malformed JSON".to_string(),
        raw_response: Some("{invalid}".to_string()),
    };
    let display = format!("{}", error);
    assert!(display.contains("Invalid response"));

    // Test ParseError
    let error = BackendError::ParseError {
        message: "Cannot parse field".to_string(),
        context: "response body".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("Parse error"));

    // Test ApiError
    let error = BackendError::ApiError {
        message: "Server error".to_string(),
        status_code: Some(500),
    };
    let display = format!("{}", error);
    assert!(display.contains("500"));

    let error = BackendError::ApiError {
        message: "Error".to_string(),
        status_code: None,
    };
    let display = format!("{}", error);
    assert!(display.contains("API error"));

    // Test ConfigurationError
    let error = BackendError::ConfigurationError {
        message: "Missing setting".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("Configuration error"));

    // Test Other
    let error = BackendError::Other {
        message: "Unknown error".to_string(),
    };
    let display = format!("{}", error);
    assert!(display.contains("Unknown error"));
}

#[test]
fn test_backend_error_implements_error_trait() {
    use aipack::ai::backend::BackendError;
    use std::error::Error;

    let error = BackendError::TimeoutError { seconds: 30 };
    let _error_trait: &dyn Error = &error;

    // Verify Display is implemented
    let display = format!("{}", error);
    assert!(!display.is_empty());
}
