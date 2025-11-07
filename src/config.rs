//! Configuration management for aipack
//!
//! This module provides a comprehensive configuration system that loads settings from
//! environment variables with sensible defaults. Configuration includes backend selection,
//! API credentials, caching options, and runtime parameters.
//!
//! # Environment Variables
//!
//! - `AIPACK_BACKEND`: Backend selection (auto|ollama|mistral) - default: "auto"
//! - `AIPACK_OLLAMA_ENDPOINT`: Ollama service URL - default: "http://localhost:11434"
//! - `AIPACK_OLLAMA_MODEL`: Ollama model name - default: "qwen:7b"
//! - `MISTRAL_API_KEY`: Mistral API key (required for Mistral backend)
//! - `AIPACK_MISTRAL_MODEL`: Mistral model - default: "mistral-small"
//! - `AIPACK_LOG_LEVEL`: Logging level - default: "info"
//! - `AIPACK_CACHE_ENABLED`: Enable caching (true|false) - default: "true"
//! - `AIPACK_CACHE_DIR`: Cache directory path - default: system temp dir + "aipack-cache"
//! - `AIPACK_REQUEST_TIMEOUT`: Timeout in seconds - default: "30"
//! - `AIPACK_MAX_CONTEXT_SIZE`: Max context bytes - default: "512000" (500KB)
//!
//! # Example
//!
//! ```no_run
//! use aipack::AipackConfig;
//! use std::env;
//!
//! // Set required environment variables for auto mode to work
//! env::set_var("MISTRAL_API_KEY", "test-key");
//!
//! // Load configuration from environment with defaults
//! let config = AipackConfig::default();
//!
//! // Validate configuration
//! config.validate().expect("Invalid configuration");
//!
//! // Get selected backend configuration
//! let backend_config = config.selected_backend_config()
//!     .expect("Failed to configure backend");
//! ```

use crate::ai::backend::BackendConfig;
use std::env;
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

/// Default values for configuration
const DEFAULT_BACKEND: &str = "auto";
const DEFAULT_OLLAMA_ENDPOINT: &str = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5-coder:7b";
const DEFAULT_MISTRAL_MODEL: &str = "mistral-small";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_CACHE_ENABLED: bool = true;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_CONTEXT_SIZE: usize = 512_000; // 500KB

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Invalid backend name provided
    #[error("Invalid backend: {0}. Valid options are: auto, ollama, mistral")]
    InvalidBackend(String),

    /// Mistral API key not found when using Mistral backend
    #[error("Mistral API key not set. Please set MISTRAL_API_KEY environment variable")]
    MissingApiKey,

    /// Invalid endpoint URL format
    #[error("Invalid endpoint URL: {0}")]
    InvalidEndpoint(String),

    /// Configuration validation failed
    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),

    /// Failed to parse configuration value
    #[error("Failed to parse {field}: {error}")]
    ParseError { field: String, error: String },

    /// Ollama endpoint is not reachable
    #[error("Ollama endpoint {0} is not reachable")]
    OllamaUnreachable(String),
}

/// Main configuration structure for aipack
///
/// This struct holds all configuration parameters needed for aipack to operate.
/// It can be constructed using `Default::default()` which loads from environment
/// variables with sensible fallback defaults.
#[derive(Debug, Clone)]
pub struct AipackConfig {
    /// Selected backend: "ollama", "mistral", or "auto"
    pub backend: String,

    /// Ollama service endpoint URL
    pub ollama_endpoint: String,

    /// Ollama model to use for inference
    pub ollama_model: String,

    /// Mistral API key (optional, required for Mistral backend)
    pub mistral_api_key: Option<String>,

    /// Mistral model to use
    pub mistral_model: String,

    /// Enable result caching
    pub cache_enabled: bool,

    /// Cache directory path
    pub cache_dir: Option<PathBuf>,

    /// Request timeout in seconds
    pub request_timeout_secs: u64,

    /// Maximum context size in bytes
    pub max_context_size: usize,

    /// Logging level (trace, debug, info, warn, error)
    pub log_level: String,
}

impl Default for AipackConfig {
    /// Creates a new configuration by loading from environment variables with defaults
    ///
    /// This will read all AIPACK_* and MISTRAL_API_KEY environment variables and
    /// fall back to sensible defaults for any missing values.
    fn default() -> Self {
        // Read backend selection
        let backend = env::var("AIPACK_BACKEND")
            .unwrap_or_else(|_| DEFAULT_BACKEND.to_string())
            .to_lowercase();

        // Ollama configuration
        let ollama_endpoint = env::var("AIPACK_OLLAMA_ENDPOINT")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_ENDPOINT.to_string());
        let ollama_model =
            env::var("AIPACK_OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_OLLAMA_MODEL.to_string());

        // Mistral configuration
        let mistral_api_key = env::var("MISTRAL_API_KEY").ok();
        let mistral_model =
            env::var("AIPACK_MISTRAL_MODEL").unwrap_or_else(|_| DEFAULT_MISTRAL_MODEL.to_string());

        // Caching configuration
        let cache_enabled = env::var("AIPACK_CACHE_ENABLED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(DEFAULT_CACHE_ENABLED);

        let cache_dir = env::var("AIPACK_CACHE_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                if cache_enabled {
                    Some(env::temp_dir().join("aipack-cache"))
                } else {
                    None
                }
            });

        // Runtime parameters
        let request_timeout_secs = env::var("AIPACK_REQUEST_TIMEOUT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

        let max_context_size = env::var("AIPACK_MAX_CONTEXT_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_CONTEXT_SIZE);

        // Logging configuration
        let log_level = env::var("AIPACK_LOG_LEVEL")
            .unwrap_or_else(|_| DEFAULT_LOG_LEVEL.to_string())
            .to_lowercase();

        Self {
            backend,
            ollama_endpoint,
            ollama_model,
            mistral_api_key,
            mistral_model,
            cache_enabled,
            cache_dir,
            request_timeout_secs,
            max_context_size,
            log_level,
        }
    }
}

impl AipackConfig {
    /// Validates the configuration
    ///
    /// Checks that:
    /// - Backend is one of: auto, ollama, mistral
    /// - Endpoint URL is valid
    /// - Required API keys are present for selected backends
    /// - Numeric values are in valid ranges
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if any validation fails
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate backend selection
        match self.backend.as_str() {
            "auto" | "ollama" | "mistral" => {}
            _ => return Err(ConfigError::InvalidBackend(self.backend.clone())),
        }

        // Validate Ollama endpoint format
        if !self.ollama_endpoint.starts_with("http://")
            && !self.ollama_endpoint.starts_with("https://")
        {
            return Err(ConfigError::InvalidEndpoint(self.ollama_endpoint.clone()));
        }

        // Validate that Mistral API key exists if using Mistral backend
        if self.backend == "mistral" && self.mistral_api_key.is_none() {
            return Err(ConfigError::MissingApiKey);
        }

        // Validate timeout is reasonable (at least 1 second, max 10 minutes)
        if self.request_timeout_secs == 0 {
            return Err(ConfigError::ValidationFailed(
                "Request timeout must be at least 1 second".to_string(),
            ));
        }
        if self.request_timeout_secs > 600 {
            return Err(ConfigError::ValidationFailed(
                "Request timeout cannot exceed 10 minutes".to_string(),
            ));
        }

        // Validate max context size is reasonable (at least 1KB, max 10MB)
        if self.max_context_size < 1024 {
            return Err(ConfigError::ValidationFailed(
                "Max context size must be at least 1KB".to_string(),
            ));
        }
        if self.max_context_size > 10_485_760 {
            return Err(ConfigError::ValidationFailed(
                "Max context size cannot exceed 10MB".to_string(),
            ));
        }

        // Validate log level
        match self.log_level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => {
                return Err(ConfigError::ValidationFailed(format!(
                    "Invalid log level: {}. Valid options: trace, debug, info, warn, error",
                    self.log_level
                )))
            }
        }

        Ok(())
    }

    /// Checks if the Ollama endpoint is reachable
    ///
    /// This performs a simple HTTP GET request to the Ollama endpoint's /api/tags
    /// endpoint to verify connectivity. This is a synchronous check and should be
    /// used sparingly.
    ///
    /// # Returns
    ///
    /// `true` if the endpoint responds successfully, `false` otherwise
    pub fn is_ollama_available(&self) -> bool {
        // Use a simple HTTP check - we'll use std lib to avoid async complexity here
        let url = format!("{}/api/tags", self.ollama_endpoint);

        // Try to make a blocking request with a short timeout
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        client.get(&url).send().is_ok()
    }

    /// Checks if Mistral API key is configured
    ///
    /// # Returns
    ///
    /// `true` if the API key is set, `false` otherwise
    pub fn has_mistral_key(&self) -> bool {
        self.mistral_api_key.is_some()
    }

    /// Gets the appropriate backend configuration based on the selected backend
    ///
    /// If "auto" is selected, this will try Ollama first (if reachable), then fall
    /// back to Mistral if an API key is available.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if:
    /// - The selected backend is invalid
    /// - Required credentials are missing
    /// - No suitable backend is available (for "auto" mode)
    pub fn selected_backend_config(&self) -> Result<BackendConfig, ConfigError> {
        match self.backend.as_str() {
            "ollama" => Ok(BackendConfig::Local {
                endpoint: self.ollama_endpoint.clone(),
                model: self.ollama_model.clone(),
                timeout_seconds: Some(self.request_timeout_secs),
                max_tokens: None,
            }),
            "mistral" => {
                let api_key = self
                    .mistral_api_key
                    .clone()
                    .ok_or(ConfigError::MissingApiKey)?;
                // Mistral can use OpenAI backend variant for now
                // This can be updated when Mistral-specific implementation is added
                Ok(BackendConfig::OpenAI {
                    api_key,
                    model: self.mistral_model.clone(),
                    organization_id: None,
                    api_endpoint: Some("https://api.mistral.ai/v1".to_string()),
                    timeout_seconds: Some(self.request_timeout_secs),
                    max_tokens: None,
                })
            }
            "auto" => {
                // Try Ollama first if available
                if self.is_ollama_available() {
                    return Ok(BackendConfig::Local {
                        endpoint: self.ollama_endpoint.clone(),
                        model: self.ollama_model.clone(),
                        timeout_seconds: Some(self.request_timeout_secs),
                        max_tokens: None,
                    });
                }

                // Fall back to Mistral if API key is available
                if let Some(api_key) = &self.mistral_api_key {
                    return Ok(BackendConfig::OpenAI {
                        api_key: api_key.clone(),
                        model: self.mistral_model.clone(),
                        organization_id: None,
                        api_endpoint: Some("https://api.mistral.ai/v1".to_string()),
                        timeout_seconds: Some(self.request_timeout_secs),
                        max_tokens: None,
                    });
                }

                // No backend available
                Err(ConfigError::ValidationFailed(
                    "Auto mode: No backend available. Ollama is not reachable and Mistral API key is not set"
                        .to_string(),
                ))
            }
            _ => Err(ConfigError::InvalidBackend(self.backend.clone())),
        }
    }

    /// Computes the cache file path for a given repository
    ///
    /// # Arguments
    ///
    /// * `repo_name` - Name or identifier of the repository
    ///
    /// # Returns
    ///
    /// Path to the cache file for this repository
    ///
    /// # Panics
    ///
    /// Panics if cache_dir is None (should only be called when caching is enabled)
    pub fn cache_path(&self, repo_name: &str) -> PathBuf {
        let cache_dir = self
            .cache_dir
            .as_ref()
            .expect("cache_path called when caching is disabled");

        // Sanitize repo name to be filesystem-safe
        let safe_name = repo_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");

        cache_dir.join(format!("{}.json", safe_name))
    }

    /// Converts configuration to a display map for output formatting
    ///
    /// # Arguments
    ///
    /// * `show_secrets` - Whether to show API keys in plain text
    ///
    /// # Returns
    ///
    /// A HashMap suitable for JSON/YAML serialization or display
    pub fn to_display_map(&self, show_secrets: bool) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();

        map.insert("backend".to_string(), self.backend.clone());
        map.insert("ollama_endpoint".to_string(), self.ollama_endpoint.clone());
        map.insert("ollama_model".to_string(), self.ollama_model.clone());
        map.insert(
            "ollama_timeout".to_string(),
            self.request_timeout_secs.to_string(),
        );

        if show_secrets {
            map.insert(
                "mistral_api_key".to_string(),
                self.mistral_api_key
                    .clone()
                    .unwrap_or_else(|| "Not set".to_string()),
            );
        } else {
            map.insert(
                "mistral_api_key".to_string(),
                if self.mistral_api_key.is_some() {
                    "****** (set)".to_string()
                } else {
                    "Not set".to_string()
                },
            );
        }

        map.insert("mistral_model".to_string(), self.mistral_model.clone());
        map.insert(
            "mistral_timeout".to_string(),
            self.request_timeout_secs.to_string(),
        );

        map.insert("cache_enabled".to_string(), self.cache_enabled.to_string());
        if let Some(ref dir) = self.cache_dir {
            map.insert("cache_dir".to_string(), dir.display().to_string());
        }

        map.insert(
            "request_timeout_secs".to_string(),
            self.request_timeout_secs.to_string(),
        );
        map.insert(
            "max_context_size".to_string(),
            self.max_context_size.to_string(),
        );
        map.insert("log_level".to_string(), self.log_level.clone());

        map
    }
}

impl fmt::Display for AipackConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Aipack Configuration:")?;
        writeln!(f, "  Backend: {}", self.backend)?;
        writeln!(f, "  Ollama Endpoint: {}", self.ollama_endpoint)?;
        writeln!(f, "  Ollama Model: {}", self.ollama_model)?;
        writeln!(
            f,
            "  Mistral API Key: {}",
            if self.mistral_api_key.is_some() {
                "Set"
            } else {
                "Not set"
            }
        )?;
        writeln!(f, "  Mistral Model: {}", self.mistral_model)?;
        writeln!(f, "  Cache Enabled: {}", self.cache_enabled)?;
        if let Some(ref dir) = self.cache_dir {
            writeln!(f, "  Cache Dir: {}", dir.display())?;
        }
        writeln!(f, "  Request Timeout: {}s", self.request_timeout_secs)?;
        writeln!(f, "  Max Context Size: {} bytes", self.max_context_size)?;
        writeln!(f, "  Log Level: {}", self.log_level)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Helper to temporarily set environment variables for testing
    struct EnvGuard {
        key: String,
        old_value: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            let old_value = env::var(key).ok();
            env::set_var(key, value);
            Self {
                key: key.to_string(),
                old_value,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.old_value {
                Some(v) => env::set_var(&self.key, v),
                None => env::remove_var(&self.key),
            }
        }
    }

    #[test]
    fn test_default_configuration() {
        // Clear relevant env vars
        let _guards = vec![
            EnvGuard::set("AIPACK_BACKEND", "auto"),
            EnvGuard::set("AIPACK_OLLAMA_ENDPOINT", DEFAULT_OLLAMA_ENDPOINT),
            EnvGuard::set("AIPACK_LOG_LEVEL", DEFAULT_LOG_LEVEL),
        ];

        let config = AipackConfig::default();

        assert_eq!(config.backend, "auto");
        assert_eq!(config.ollama_endpoint, DEFAULT_OLLAMA_ENDPOINT);
        assert_eq!(config.ollama_model, DEFAULT_OLLAMA_MODEL);
        assert_eq!(config.mistral_model, DEFAULT_MISTRAL_MODEL);
        assert_eq!(config.cache_enabled, DEFAULT_CACHE_ENABLED);
        assert_eq!(config.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
        assert_eq!(config.max_context_size, DEFAULT_MAX_CONTEXT_SIZE);
        assert_eq!(config.log_level, DEFAULT_LOG_LEVEL);
    }

    #[test]
    fn test_environment_variable_parsing() {
        let _guards = vec![
            EnvGuard::set("AIPACK_BACKEND", "ollama"),
            EnvGuard::set("AIPACK_OLLAMA_ENDPOINT", "http://custom:11434"),
            EnvGuard::set("AIPACK_OLLAMA_MODEL", "custom-model"),
            EnvGuard::set("AIPACK_LOG_LEVEL", "debug"),
            EnvGuard::set("AIPACK_CACHE_ENABLED", "false"),
            EnvGuard::set("AIPACK_REQUEST_TIMEOUT", "60"),
            EnvGuard::set("AIPACK_MAX_CONTEXT_SIZE", "1024000"),
        ];

        let config = AipackConfig::default();

        assert_eq!(config.backend, "ollama");
        assert_eq!(config.ollama_endpoint, "http://custom:11434");
        assert_eq!(config.ollama_model, "custom-model");
        assert_eq!(config.log_level, "debug");
        assert!(!config.cache_enabled);
        assert_eq!(config.request_timeout_secs, 60);
        assert_eq!(config.max_context_size, 1_024_000);
    }

    #[test]
    fn test_configuration_validation_valid() {
        let config = AipackConfig {
            backend: "ollama".to_string(),
            ollama_endpoint: "http://localhost:11434".to_string(),
            ollama_model: "qwen:7b".to_string(),
            mistral_api_key: None,
            mistral_model: "mistral-small".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_configuration_validation_invalid_backend() {
        let mut config = AipackConfig::default();
        config.backend = "invalid".to_string();

        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(ConfigError::InvalidBackend(ref backend)) => {
                assert_eq!(backend, "invalid");
            }
            _ => panic!("Expected InvalidBackend error"),
        }
    }

    #[test]
    fn test_configuration_validation_missing_mistral_key() {
        let mut config = AipackConfig::default();
        config.backend = "mistral".to_string();
        config.mistral_api_key = None;

        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(ConfigError::MissingApiKey) => {}
            _ => panic!("Expected MissingApiKey error"),
        }
    }

    #[test]
    fn test_configuration_validation_invalid_endpoint() {
        let mut config = AipackConfig::default();
        config.ollama_endpoint = "not-a-url".to_string();

        let result = config.validate();
        assert!(result.is_err());
        match result {
            Err(ConfigError::InvalidEndpoint(_)) => {}
            _ => panic!("Expected InvalidEndpoint error"),
        }
    }

    #[test]
    fn test_configuration_validation_invalid_timeout() {
        let mut config = AipackConfig::default();
        config.request_timeout_secs = 0;

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_configuration_validation_invalid_log_level() {
        let mut config = AipackConfig::default();
        config.log_level = "invalid".to_string();

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_has_mistral_key() {
        let mut config = AipackConfig::default();
        assert!(!config.has_mistral_key());

        config.mistral_api_key = Some("test-key".to_string());
        assert!(config.has_mistral_key());
    }

    #[test]
    fn test_selected_backend_config_ollama() {
        let config = AipackConfig {
            backend: "ollama".to_string(),
            ollama_endpoint: "http://localhost:11434".to_string(),
            ollama_model: "qwen:7b".to_string(),
            mistral_api_key: None,
            mistral_model: "mistral-small".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
        };

        let backend = config.selected_backend_config().unwrap();
        match backend {
            BackendConfig::Local {
                endpoint, model, ..
            } => {
                assert_eq!(endpoint, "http://localhost:11434");
                assert_eq!(model, "qwen:7b");
            }
            _ => panic!("Expected Local backend"),
        }
    }

    #[test]
    fn test_selected_backend_config_mistral() {
        let config = AipackConfig {
            backend: "mistral".to_string(),
            ollama_endpoint: "http://localhost:11434".to_string(),
            ollama_model: "qwen:7b".to_string(),
            mistral_api_key: Some("test-key".to_string()),
            mistral_model: "mistral-small".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
        };

        let backend = config.selected_backend_config().unwrap();
        match backend {
            BackendConfig::OpenAI { api_key, model, .. } => {
                assert_eq!(api_key, "test-key");
                assert_eq!(model, "mistral-small");
            }
            _ => panic!("Expected OpenAI backend (for Mistral)"),
        }
    }

    #[test]
    fn test_selected_backend_config_mistral_missing_key() {
        let config = AipackConfig {
            backend: "mistral".to_string(),
            ollama_endpoint: "http://localhost:11434".to_string(),
            ollama_model: "qwen:7b".to_string(),
            mistral_api_key: None,
            mistral_model: "mistral-small".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
        };

        let result = config.selected_backend_config();
        assert!(result.is_err());
        match result {
            Err(ConfigError::MissingApiKey) => {}
            _ => panic!("Expected MissingApiKey error"),
        }
    }

    #[test]
    fn test_cache_path() {
        let config = AipackConfig {
            backend: "ollama".to_string(),
            ollama_endpoint: "http://localhost:11434".to_string(),
            ollama_model: "qwen:7b".to_string(),
            mistral_api_key: None,
            mistral_model: "mistral-small".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
        };

        let path = config.cache_path("myrepo");
        assert_eq!(path, PathBuf::from("/tmp/cache/myrepo.json"));
    }

    #[test]
    fn test_cache_path_sanitizes_special_chars() {
        let config = AipackConfig {
            backend: "ollama".to_string(),
            ollama_endpoint: "http://localhost:11434".to_string(),
            ollama_model: "qwen:7b".to_string(),
            mistral_api_key: None,
            mistral_model: "mistral-small".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
        };

        let path = config.cache_path("user/repo:branch");
        assert_eq!(path, PathBuf::from("/tmp/cache/user_repo_branch.json"));
    }

    #[test]
    fn test_config_display() {
        let config = AipackConfig::default();
        let display = format!("{}", config);
        assert!(display.contains("Aipack Configuration:"));
        assert!(display.contains("Backend:"));
    }
}
