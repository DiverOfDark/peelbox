use crate::error::BackendError;
use genai::adapter::AdapterKind;
use std::env;
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5-coder:7b";
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_CACHE_ENABLED: bool = true;
const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_CONTEXT_SIZE: usize = 512_000;
const DEFAULT_MAX_TOOL_ITERATIONS: usize = 10;
const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_FILE_SIZE_BYTES: usize = 1_048_576; // 1MB
const DEFAULT_MAX_TOKENS: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionMode {
    Full,
    StaticOnly,
    LLMOnly,
}

impl DetectionMode {
    pub fn from_env() -> Self {
        let mode = env::var("PEELBOX_DETECTION_MODE").ok();

        match mode.as_deref() {
            Some(m) if m.eq_ignore_ascii_case("static") => DetectionMode::StaticOnly,
            Some(m) if m.eq_ignore_ascii_case("llm") => DetectionMode::LLMOnly,
            _ => DetectionMode::Full,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Provider not specified. Set PEELBOX_PROVIDER environment variable (ollama|openai|claude|gemini|grok|groq)")]
    MissingProvider,

    #[error("Invalid provider: {0}. Valid options: ollama, openai, claude, gemini, grok, groq")]
    InvalidProvider(String),

    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),

    #[error("Failed to parse {field}: {error}")]
    ParseError { field: String, error: String },

    #[error("Backend initialization failed: {0}")]
    BackendInitError(#[from] BackendError),
}

#[derive(Debug, Clone)]
pub struct PeelboxConfig {
    pub provider: AdapterKind,
    pub model: String,
    pub cache_enabled: bool,
    pub cache_dir: Option<PathBuf>,
    pub request_timeout_secs: u64,
    pub max_context_size: usize,
    pub log_level: String,
    pub max_tool_iterations: usize,
    pub tool_timeout_secs: u64,
    pub max_file_size_bytes: usize,
    pub max_tokens: usize,
}

impl Default for PeelboxConfig {
    fn default() -> Self {
        let provider = env::var("PEELBOX_PROVIDER")
            .ok()
            .and_then(|s| match s.to_lowercase().as_str() {
                "ollama" => Some(AdapterKind::Ollama),
                "openai" => Some(AdapterKind::OpenAI),
                "claude" => Some(AdapterKind::Anthropic),
                "gemini" => Some(AdapterKind::Gemini),
                "grok" => Some(AdapterKind::Xai),
                "groq" => Some(AdapterKind::Groq),
                _ => None,
            })
            .unwrap_or(AdapterKind::Ollama);

        let model = env::var("PEELBOX_MODEL")
            .ok()
            .unwrap_or_else(|| match provider {
                AdapterKind::Ollama => DEFAULT_OLLAMA_MODEL.to_string(),
                _ => "default-model".to_string(),
            });

        let cache_enabled = env::var("PEELBOX_CACHE_ENABLED")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(DEFAULT_CACHE_ENABLED);

        let cache_dir = env::var("PEELBOX_CACHE_DIR")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                if cache_enabled {
                    Some(env::temp_dir().join("peelbox-cache"))
                } else {
                    None
                }
            });

        let request_timeout_secs = env::var("PEELBOX_REQUEST_TIMEOUT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS);

        let max_context_size = env::var("PEELBOX_MAX_CONTEXT_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_CONTEXT_SIZE);

        let log_level = env::var("PEELBOX_LOG_LEVEL")
            .unwrap_or_else(|_| DEFAULT_LOG_LEVEL.to_string())
            .to_lowercase();

        let max_tool_iterations = env::var("PEELBOX_MAX_TOOL_ITERATIONS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_TOOL_ITERATIONS);

        let tool_timeout_secs = env::var("PEELBOX_TOOL_TIMEOUT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_TOOL_TIMEOUT_SECS);

        let max_file_size_bytes = env::var("PEELBOX_MAX_FILE_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_FILE_SIZE_BYTES);

        let max_tokens = env::var("PEELBOX_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_TOKENS);

        Self {
            provider,
            model,
            cache_enabled,
            cache_dir,
            request_timeout_secs,
            max_context_size,
            log_level,
            max_tool_iterations,
            tool_timeout_secs,
            max_file_size_bytes,
            max_tokens,
        }
    }
}

impl PeelboxConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.request_timeout_secs == 0 {
            return Err(ConfigError::ValidationFailed(
                "Request timeout must be at least 1 second".to_string(),
            ));
        }
        if self.request_timeout_secs > 3600 {
            return Err(ConfigError::ValidationFailed(
                "Request timeout cannot exceed 1 hour".to_string(),
            ));
        }

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

        match self.log_level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => {
                return Err(ConfigError::ValidationFailed(format!(
                    "Invalid log level: {}. Valid options: trace, debug, info, warn, error",
                    self.log_level
                )))
            }
        }

        if self.max_tool_iterations == 0 {
            return Err(ConfigError::ValidationFailed(
                "Max tool iterations must be at least 1".to_string(),
            ));
        }
        if self.max_tool_iterations > 50 {
            return Err(ConfigError::ValidationFailed(
                "Max tool iterations cannot exceed 50".to_string(),
            ));
        }

        if self.tool_timeout_secs == 0 {
            return Err(ConfigError::ValidationFailed(
                "Tool timeout must be at least 1 second".to_string(),
            ));
        }
        if self.tool_timeout_secs > 300 {
            return Err(ConfigError::ValidationFailed(
                "Tool timeout cannot exceed 5 minutes".to_string(),
            ));
        }

        if self.max_file_size_bytes < 1024 {
            return Err(ConfigError::ValidationFailed(
                "Max file size must be at least 1KB".to_string(),
            ));
        }
        if self.max_file_size_bytes > 10_485_760 {
            return Err(ConfigError::ValidationFailed(
                "Max file size cannot exceed 10MB".to_string(),
            ));
        }

        if self.max_tokens < 512 {
            return Err(ConfigError::ValidationFailed(
                "Max tokens must be at least 512".to_string(),
            ));
        }
        if self.max_tokens > 128_000 {
            return Err(ConfigError::ValidationFailed(
                "Max tokens cannot exceed 128000".to_string(),
            ));
        }

        Ok(())
    }

    pub fn cache_path(&self, repo_name: &str) -> PathBuf {
        let cache_dir = self
            .cache_dir
            .as_ref()
            .expect("cache_path called when caching is disabled");

        let safe_name = repo_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
        cache_dir.join(format!("{}.json", safe_name))
    }

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
        map.insert(
            "max_tool_iterations".to_string(),
            self.max_tool_iterations.to_string(),
        );
        map.insert(
            "tool_timeout_secs".to_string(),
            self.tool_timeout_secs.to_string(),
        );
        map.insert(
            "max_file_size_bytes".to_string(),
            self.max_file_size_bytes.to_string(),
        );
        map.insert("max_tokens".to_string(), self.max_tokens.to_string());

        map
    }
}

impl fmt::Display for PeelboxConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Peelbox Configuration:")?;
        writeln!(f, "  Provider: {:?}", self.provider)?;
        writeln!(f, "  Model: {}", self.model)?;
        writeln!(f, "  Cache Enabled: {}", self.cache_enabled)?;
        if let Some(ref dir) = self.cache_dir {
            writeln!(f, "  Cache Dir: {}", dir.display())?;
        }
        writeln!(f, "  Request Timeout: {}s", self.request_timeout_secs)?;
        writeln!(f, "  Max Context Size: {} bytes", self.max_context_size)?;
        writeln!(f, "  Log Level: {}", self.log_level)?;
        writeln!(f, "  Max Tool Iterations: {}", self.max_tool_iterations)?;
        writeln!(f, "  Tool Timeout: {}s", self.tool_timeout_secs)?;
        writeln!(f, "  Max File Size: {} bytes", self.max_file_size_bytes)?;
        writeln!(f, "  Max Tokens: {}", self.max_tokens)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

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
        let _guards = [
            EnvGuard::set("PEELBOX_PROVIDER", "ollama"),
            EnvGuard::set("PEELBOX_LOG_LEVEL", DEFAULT_LOG_LEVEL),
        ];

        let config = PeelboxConfig::default();

        assert!(matches!(config.provider, AdapterKind::Ollama));
        assert_eq!(config.model, DEFAULT_OLLAMA_MODEL);
        assert_eq!(config.cache_enabled, DEFAULT_CACHE_ENABLED);
        assert_eq!(config.request_timeout_secs, DEFAULT_REQUEST_TIMEOUT_SECS);
        assert_eq!(config.max_context_size, DEFAULT_MAX_CONTEXT_SIZE);
        assert_eq!(config.log_level, DEFAULT_LOG_LEVEL);
        assert_eq!(config.max_tool_iterations, DEFAULT_MAX_TOOL_ITERATIONS);
        assert_eq!(config.tool_timeout_secs, DEFAULT_TOOL_TIMEOUT_SECS);
        assert_eq!(config.max_file_size_bytes, DEFAULT_MAX_FILE_SIZE_BYTES);
        assert_eq!(config.max_tokens, DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn test_environment_variable_parsing() {
        let _guards = [
            EnvGuard::set("PEELBOX_PROVIDER", "claude"),
            EnvGuard::set("PEELBOX_MODEL", "custom-model"),
            EnvGuard::set("PEELBOX_LOG_LEVEL", "debug"),
            EnvGuard::set("PEELBOX_CACHE_ENABLED", "false"),
            EnvGuard::set("PEELBOX_REQUEST_TIMEOUT", "60"),
            EnvGuard::set("PEELBOX_MAX_CONTEXT_SIZE", "1024000"),
            EnvGuard::set("PEELBOX_MAX_TOKENS", "4096"),
        ];

        let config = PeelboxConfig::default();

        assert!(matches!(config.provider, AdapterKind::Anthropic));
        assert_eq!(config.model, "custom-model");
        assert_eq!(config.log_level, "debug");
        assert!(!config.cache_enabled);
        assert_eq!(config.request_timeout_secs, 60);
        assert_eq!(config.max_context_size, 1_024_000);
        assert_eq!(config.max_tokens, 4096);
    }

    #[test]
    fn test_configuration_validation_valid() {
        let config = PeelboxConfig {
            provider: AdapterKind::Ollama,
            model: "qwen:7b".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
            max_tool_iterations: 10,
            tool_timeout_secs: 30,
            max_file_size_bytes: 1_048_576,
            max_tokens: 8192,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_configuration_validation_invalid_timeout() {
        let config = PeelboxConfig {
            request_timeout_secs: 0,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_configuration_validation_invalid_log_level() {
        let config = PeelboxConfig {
            log_level: "invalid".to_string(),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_configuration_validation_invalid_max_tokens_too_low() {
        let config = PeelboxConfig {
            max_tokens: 256,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("512"));
    }

    #[test]
    fn test_configuration_validation_invalid_max_tokens_too_high() {
        let config = PeelboxConfig {
            max_tokens: 200_000,
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("128000"));
    }

    #[test]
    fn test_cache_path() {
        let config = PeelboxConfig {
            provider: AdapterKind::Ollama,
            model: "qwen:7b".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
            max_tool_iterations: 10,
            tool_timeout_secs: 30,
            max_file_size_bytes: 1_048_576,
            max_tokens: 8192,
        };

        let path = config.cache_path("myrepo");
        assert_eq!(path, PathBuf::from("/tmp/cache/myrepo.json"));
    }

    #[test]
    fn test_cache_path_sanitizes_special_chars() {
        let config = PeelboxConfig {
            provider: AdapterKind::Ollama,
            model: "qwen:7b".to_string(),
            cache_enabled: true,
            cache_dir: Some(PathBuf::from("/tmp/cache")),
            request_timeout_secs: 30,
            max_context_size: 512_000,
            log_level: "info".to_string(),
            max_tool_iterations: 10,
            tool_timeout_secs: 30,
            max_file_size_bytes: 1_048_576,
            max_tokens: 8192,
        };

        let path = config.cache_path("user/repo:branch");
        assert_eq!(path, PathBuf::from("/tmp/cache/user_repo_branch.json"));
    }

    #[test]
    fn test_config_display() {
        let config = PeelboxConfig::default();
        let display = format!("{}", config);
        assert!(display.contains("Peelbox Configuration:"));
        assert!(display.contains("Provider:"));
    }

    #[test]
    #[serial]
    fn test_detection_mode_from_env_default() {
        env::remove_var("PEELBOX_DETECTION_MODE");

        let mode = DetectionMode::from_env();
        assert_eq!(mode, DetectionMode::Full);
    }

    #[test]
    #[serial]
    fn test_detection_mode_from_env_static() {
        let _guard = EnvGuard::set("PEELBOX_DETECTION_MODE", "static");

        let mode = DetectionMode::from_env();
        assert_eq!(mode, DetectionMode::StaticOnly);
    }

    #[test]
    #[serial]
    fn test_detection_mode_from_env_llm() {
        let _guard = EnvGuard::set("PEELBOX_DETECTION_MODE", "llm");

        let mode = DetectionMode::from_env();
        assert_eq!(mode, DetectionMode::LLMOnly);
    }

    #[test]
    #[serial]
    fn test_detection_mode_from_env_full() {
        let _guard = EnvGuard::set("PEELBOX_DETECTION_MODE", "full");

        let mode = DetectionMode::from_env();
        assert_eq!(mode, DetectionMode::Full);
    }

    #[test]
    #[serial]
    fn test_detection_mode_from_env_case_insensitive() {
        let _guard = EnvGuard::set("PEELBOX_DETECTION_MODE", "STATIC");

        let mode = DetectionMode::from_env();
        assert_eq!(mode, DetectionMode::StaticOnly);
    }

    #[test]
    #[serial]
    fn test_detection_mode_from_env_invalid_defaults_to_full() {
        let _guard = EnvGuard::set("PEELBOX_DETECTION_MODE", "invalid");

        let mode = DetectionMode::from_env();
        assert_eq!(mode, DetectionMode::Full);
    }
}
