//! Configuration management for aipack
//!
//! This module provides a comprehensive configuration system that loads settings from
//! environment variables with sensible defaults. Configuration includes backend selection,
//! API credentials, caching options, and runtime parameters.
//!
//! # Environment Variables
//!
//! ## Aipack Configuration
//! - `AIPACK_PROVIDER`: Provider selection (ollama|openai|claude|gemini|grok|groq) - **required**
//! - `AIPACK_OLLAMA_MODEL`: Ollama model name - default: "qwen2.5-coder:7b"
//! - `AIPACK_LOG_LEVEL`: Logging level - default: "info"
//! - `AIPACK_CACHE_ENABLED`: Enable caching (true|false) - default: "true"
//! - `AIPACK_CACHE_DIR`: Cache directory path - default: system temp dir + "aipack-cache"
//! - `AIPACK_REQUEST_TIMEOUT`: Timeout in seconds - default: "30"
//! - `AIPACK_MAX_CONTEXT_SIZE`: Max context bytes - default: "512000" (500KB)
//!
//! ## GenAI Provider Configuration
//! These environment variables are read directly by the genai library:
//! - **Ollama**: `OLLAMA_HOST` (default: http://localhost:11434)
//! - **OpenAI**: `OPENAI_API_KEY` (required), `OPENAI_API_BASE` (optional)
//! - **Claude**: `ANTHROPIC_API_KEY` (required), `ANTHROPIC_BASE_URL` (optional)
//! - **Gemini**: `GOOGLE_API_KEY` (required)
//! - **Grok**: `XAI_API_KEY` (required)
//! - **Groq**: `GROQ_API_KEY` (required)
//!
//! # Example
//!
//! ```no_run
//! use aipack::AipackConfig;
//! use std::env;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Set required environment variables
//! env::set_var("AIPACK_PROVIDER", "ollama");
//! env::set_var("OLLAMA_HOST", "http://localhost:11434"); // Optional, has default
//!
//! // Load configuration from environment with defaults
//! let config = AipackConfig::default();
//!
//! // Validate configuration
//! config.validate().expect("Invalid configuration");
//!
//! // Create backend directly from configuration
//! let backend = config.create_backend().await?;
//! # Ok(())
//! # }
//! ```

use crate::ai::genai_backend::{BackendError, GenAIBackend, Provider};
use std::env;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Default values for configuration
const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5-coder:7b";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_CACHE_ENABLED: bool = true;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_CONTEXT_SIZE: usize = 512_000; // 500KB

/// Configuration errors
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Provider not specified
    #[error("Provider not specified. Set AIPACK_PROVIDER environment variable (ollama|openai|claude|gemini|grok|groq)")]
    MissingProvider,

    /// Invalid provider name
    #[error("Invalid provider: {0}. Valid options: ollama, openai, claude, gemini, grok, groq")]
    InvalidProvider(String),

    /// Configuration validation failed
    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),

    /// Failed to parse configuration value
    #[error("Failed to parse {field}: {error}")]
    ParseError { field: String, error: String },

    /// Backend initialization failed
    #[error("Backend initialization failed: {0}")]
    BackendInitError(#[from] BackendError),
}

/// Main configuration structure for aipack
///
/// This struct holds all configuration parameters needed for aipack to operate.
/// It can be constructed using `Default::default()` which loads from environment
/// variables with sensible fallback defaults.
#[derive(Debug, Clone)]
pub struct AipackConfig {
    /// LLM provider (from genai)
    pub provider: Provider,

    /// Model name to use for inference (provider-specific)
    pub model: String,

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
    /// This will read AIPACK_* environment variables and fall back to sensible defaults
    /// for any missing values. Provider-specific configuration (API keys, endpoints) should
    /// be set via standard genai environment variables (OLLAMA_HOST, OPENAI_API_KEY, etc.).
    fn default() -> Self {
        // Read provider selection (required)
        let provider = env::var("AIPACK_PROVIDER")
            .ok()
            .and_then(|s| match s.to_lowercase().as_str() {
                "ollama" => Some(Provider::Ollama),
                "openai" => Some(Provider::OpenAI),
                "claude" => Some(Provider::Claude),
                "gemini" => Some(Provider::Gemini),
                "grok" => Some(Provider::Grok),
                "groq" => Some(Provider::Groq),
                _ => None,
            })
            .unwrap_or(Provider::Ollama); // Default to Ollama if not specified

        // Model configuration - provider-specific defaults
        let model = env::var("AIPACK_MODEL")
            .ok()
            .unwrap_or_else(|| match provider {
                Provider::Ollama => DEFAULT_OLLAMA_MODEL.to_string(),
                _ => "default-model".to_string(),
            });

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
            provider,
            model,
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
    /// - Numeric values are in valid ranges
    /// - Log level is valid
    ///
    /// Provider-specific validation (API keys, endpoints) is handled by genai
    /// when the backend is initialized.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if any validation fails
    pub fn validate(&self) -> Result<(), ConfigError> {
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

    /// Creates an LLM backend based on the configured provider
    ///
    /// This method directly instantiates a GenAI backend using the configured provider.
    /// Provider-specific configuration (API keys, endpoints) should be set via standard
    /// genai environment variables (OLLAMA_HOST, OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.).
    ///
    /// # Returns
    ///
    /// An `Arc<GenAIBackend>` ready for detection operations
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if backend initialization fails (missing API keys,
    /// unreachable endpoints, etc.).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use aipack::AipackConfig;
    /// use std::env;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// // Set provider-specific env vars
    /// env::set_var("AIPACK_PROVIDER", "ollama");
    /// env::set_var("OLLAMA_HOST", "http://localhost:11434");
    ///
    /// let config = AipackConfig::default();
    /// let backend = config.create_backend().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_backend(&self) -> Result<Arc<GenAIBackend>, ConfigError> {
        let timeout = Duration::from_secs(self.request_timeout_secs);

        // Use the configured model for all providers
        let model = self.model.clone();

        let client = GenAIBackend::with_config(self.provider, model, Some(timeout), None).await?;

        Ok(Arc::new(client))
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
    /// # Returns
    ///
    /// A HashMap suitable for JSON/YAML serialization or display
    pub fn to_display_map(&self) -> std::collections::HashMap<String, String> {
        let mut map = std::collections::HashMap::new();

        map.insert("provider".to_string(), format!("{:?}", self.provider));
        map.insert("model".to_string(), self.model.clone());
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
        writeln!(f, "  Provider: {:?}", self.provider)?;
        writeln!(f, "  Model: {}", self.model)?;
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
            EnvGuard::set("AIPACK_PROVIDER", "ollama"),
            EnvGuard::set("AIPACK_LOG_LEVEL", DEFAULT_LOG_LEVEL),
        ];

        let config = AipackConfig::default();

        assert!(matches!(config.provider, Provider::Ollama));
        assert_eq!(config.model, DEFAULT_OLLAMA_MODEL);
        assert_eq!(config.cache_enabled, DEFAULT_CACHE_ENABLED);
        assert_eq!(config.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
        assert_eq!(config.max_context_size, DEFAULT_MAX_CONTEXT_SIZE);
        assert_eq!(config.log_level, DEFAULT_LOG_LEVEL);
    }

    #[test]
    fn test_environment_variable_parsing() {
        let _guards = vec![
            EnvGuard::set("AIPACK_PROVIDER", "claude"),
            EnvGuard::set("AIPACK_MODEL", "custom-model"),
            EnvGuard::set("AIPACK_LOG_LEVEL", "debug"),
            EnvGuard::set("AIPACK_CACHE_ENABLED", "false"),
            EnvGuard::set("AIPACK_REQUEST_TIMEOUT", "60"),
            EnvGuard::set("AIPACK_MAX_CONTEXT_SIZE", "1024000"),
        ];

        let config = AipackConfig::default();

        assert!(matches!(config.provider, Provider::Claude));
        assert_eq!(config.model, "custom-model");
        assert_eq!(config.log_level, "debug");
        assert!(!config.cache_enabled);
        assert_eq!(config.request_timeout_secs, 60);
        assert_eq!(config.max_context_size, 1_024_000);
    }

    #[test]
    fn test_configuration_validation_valid() {
        let config = AipackConfig {
            provider: Provider::Ollama,
            model: "qwen:7b".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
        };

        assert!(config.validate().is_ok());
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
    fn test_cache_path() {
        let config = AipackConfig {
            provider: Provider::Ollama,
            model: "qwen:7b".to_string(),
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
            provider: Provider::Ollama,
            model: "qwen:7b".to_string(),
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
        assert!(display.contains("Provider:"));
    }
}
