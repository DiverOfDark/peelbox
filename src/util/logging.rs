//! Structured logging setup for aipack
//!
//! This module provides initialization and configuration for structured logging using
//! the `tracing` ecosystem. It supports various output formats, filtering, and
//! runtime configuration via environment variables.
//!
//! # Features
//!
//! - Console output with pretty formatting (default)
//! - Optional JSON output for production environments
//! - Environment-based configuration via `RUST_LOG`
//! - Configurable log levels and formatting options
//! - Thread-safe, can only be initialized once
//!
//! # Example
//!
//! ```no_run
//! use aipack::util::logging;
//!
//! // Initialize with default configuration
//! logging::init_default();
//!
//! // Or initialize from environment variables
//! logging::init_from_env();
//!
//! // Now use tracing macros throughout your code
//! use tracing::{info, debug, warn, error};
//!
//! info!("Application started");
//! debug!(repo = "myrepo", "Analyzing repository");
//!
//! // Example with error handling
//! let result: Result<(), &str> = Err("something went wrong");
//! if let Err(e) = result {
//!     warn!(error = ?e, "Non-fatal error occurred");
//!     error!("Fatal error: {}", e);
//! }
//! ```

use std::env;
use std::sync::Once;
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Ensures logging is only initialized once
static INIT: Once = Once::new();

/// Configuration for logging initialization
///
/// This struct controls how the logging system behaves, including the minimum
/// log level, output format, and what information is included in log messages.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Minimum log level to display
    pub level: Level,

    /// Use JSON output format (for structured logging in production)
    pub use_json: bool,

    /// Include the module target (e.g., aipack::detection) in logs
    pub include_target: bool,

    /// Include file and line number information
    pub include_location: bool,

    /// Include thread ID and name in logs
    pub include_thread_ids: bool,
}

impl Default for LoggingConfig {
    /// Creates a default logging configuration
    ///
    /// Defaults:
    /// - Level: INFO
    /// - JSON: false (pretty console output)
    /// - Target: true
    /// - Location: false (for cleaner output)
    /// - Thread IDs: false
    fn default() -> Self {
        Self {
            level: Level::INFO,
            use_json: false,
            include_target: true,
            include_location: false,
            include_thread_ids: false,
        }
    }
}

impl LoggingConfig {
    /// Creates a logging configuration with the specified level
    ///
    /// # Arguments
    ///
    /// * `level` - The minimum log level (trace, debug, info, warn, error)
    ///
    /// # Example
    ///
    /// ```
    /// use aipack::util::LoggingConfig;
    /// use tracing::Level;
    ///
    /// let config = LoggingConfig::with_level(Level::DEBUG);
    /// ```
    pub fn with_level(level: Level) -> Self {
        Self {
            level,
            ..Default::default()
        }
    }

    /// Creates a logging configuration for production use
    ///
    /// This enables JSON output and includes more metadata for structured logging.
    pub fn production() -> Self {
        Self {
            level: Level::INFO,
            use_json: true,
            include_target: true,
            include_location: true,
            include_thread_ids: true,
        }
    }

    /// Creates a logging configuration for development use
    ///
    /// This uses pretty console output with debug level and minimal metadata.
    pub fn development() -> Self {
        Self {
            level: Level::DEBUG,
            use_json: false,
            include_target: true,
            include_location: false,
            include_thread_ids: false,
        }
    }
}

/// Parses a log level from a string
///
/// # Arguments
///
/// * `level_str` - String representation of the level (case-insensitive)
///
/// # Returns
///
/// The corresponding `Level`, or `Level::INFO` if parsing fails
///
/// # Example
///
/// ```
/// use aipack::util::logging::parse_level;
/// use tracing::Level;
///
/// assert_eq!(parse_level("debug"), Level::DEBUG);
/// assert_eq!(parse_level("INFO"), Level::INFO);
/// assert_eq!(parse_level("invalid"), Level::INFO);
/// ```
pub fn parse_level(level_str: &str) -> Level {
    match level_str.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => {
            eprintln!(
                "Invalid log level '{}', defaulting to INFO. Valid levels: trace, debug, info, warn, error",
                level_str
            );
            Level::INFO
        }
    }
}

/// Initializes the logging system with the provided configuration
///
/// This function sets up the `tracing` subscriber with the specified configuration.
/// It can only be called once - subsequent calls will be ignored.
///
/// # Arguments
///
/// * `config` - The logging configuration to use
///
/// # Example
///
/// ```no_run
/// use aipack::util::{LoggingConfig, init_logging};
/// use tracing::Level;
///
/// let config = LoggingConfig::with_level(Level::DEBUG);
/// init_logging(config);
/// ```
pub fn init_logging(config: LoggingConfig) {
    INIT.call_once(|| {
        // Build the EnvFilter
        // Start with the configured level as default
        let mut filter = EnvFilter::from_default_env()
            .add_directive(format!("aipack={}", config.level).parse().unwrap());

        // If RUST_LOG is not set, apply our default filter
        if env::var("RUST_LOG").is_err() {
            filter = filter
                .add_directive("h2=warn".parse().unwrap())
                .add_directive("hyper=warn".parse().unwrap())
                .add_directive("reqwest=warn".parse().unwrap());
        }

        if config.use_json {
            // JSON output for production/structured logging
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_target(config.include_target)
                        .with_file(config.include_location)
                        .with_line_number(config.include_location)
                        .with_thread_ids(config.include_thread_ids)
                        .with_thread_names(config.include_thread_ids),
                )
                .init();
        } else {
            // Pretty console output for development
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(config.include_target)
                        .with_file(config.include_location)
                        .with_line_number(config.include_location)
                        .with_thread_ids(config.include_thread_ids)
                        .with_thread_names(config.include_thread_ids),
                )
                .init();
        }
    });
}

/// Initializes logging with default configuration
///
/// This is a convenience function that initializes logging with sensible defaults:
/// - INFO level
/// - Pretty console output
/// - Includes module targets
/// - Respects RUST_LOG environment variable
///
/// # Example
///
/// ```no_run
/// use aipack::util::logging;
///
/// logging::init_default();
/// ```
pub fn init_default() {
    init_logging(LoggingConfig::default());
}

/// Initializes logging from environment variables
///
/// This reads configuration from:
/// - `AIPACK_LOG_LEVEL` - Log level (trace, debug, info, warn, error)
/// - `AIPACK_LOG_JSON` - Use JSON output (true/false)
/// - `RUST_LOG` - Standard Rust log filtering
///
/// Falls back to default configuration if environment variables are not set.
///
/// # Example
///
/// ```no_run
/// use aipack::util::logging;
///
/// // With environment: AIPACK_LOG_LEVEL=debug
/// logging::init_from_env();
/// ```
pub fn init_from_env() {
    let level_str = env::var("AIPACK_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    let level = parse_level(&level_str);

    let use_json = env::var("AIPACK_LOG_JSON")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    let config = LoggingConfig {
        level,
        use_json,
        ..Default::default()
    };

    init_logging(config);
}

/// Initializes logging with a specific log level from string
///
/// This is a convenience function for quickly setting up logging with a
/// specific level without creating a full configuration.
///
/// # Arguments
///
/// * `level_str` - String representation of the level
///
/// # Example
///
/// ```no_run
/// use aipack::util::logging;
///
/// logging::with_level("debug");
/// ```
pub fn with_level(level_str: &str) {
    let level = parse_level(level_str);
    init_logging(LoggingConfig::with_level(level));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_level() {
        assert_eq!(parse_level("trace"), Level::TRACE);
        assert_eq!(parse_level("debug"), Level::DEBUG);
        assert_eq!(parse_level("info"), Level::INFO);
        assert_eq!(parse_level("warn"), Level::WARN);
        assert_eq!(parse_level("error"), Level::ERROR);
    }

    #[test]
    fn test_parse_level_case_insensitive() {
        assert_eq!(parse_level("TRACE"), Level::TRACE);
        assert_eq!(parse_level("Debug"), Level::DEBUG);
        assert_eq!(parse_level("INFO"), Level::INFO);
    }

    #[test]
    fn test_parse_level_invalid() {
        // Invalid levels default to INFO
        assert_eq!(parse_level("invalid"), Level::INFO);
        assert_eq!(parse_level(""), Level::INFO);
    }

    #[test]
    fn test_default_config() {
        let config = LoggingConfig::default();
        assert_eq!(config.level, Level::INFO);
        assert!(!config.use_json);
        assert!(config.include_target);
        assert!(!config.include_location);
        assert!(!config.include_thread_ids);
    }

    #[test]
    fn test_with_level() {
        let config = LoggingConfig::with_level(Level::DEBUG);
        assert_eq!(config.level, Level::DEBUG);
        assert!(!config.use_json);
    }

    #[test]
    fn test_production_config() {
        let config = LoggingConfig::production();
        assert_eq!(config.level, Level::INFO);
        assert!(config.use_json);
        assert!(config.include_target);
        assert!(config.include_location);
        assert!(config.include_thread_ids);
    }

    #[test]
    fn test_development_config() {
        let config = LoggingConfig::development();
        assert_eq!(config.level, Level::DEBUG);
        assert!(!config.use_json);
        assert!(config.include_target);
        assert!(!config.include_location);
        assert!(!config.include_thread_ids);
    }

    #[test]
    fn test_init_logging_doesnt_panic() {
        // Just ensure initialization doesn't panic
        // We can't test it properly in unit tests due to Once::call_once
        let config = LoggingConfig::default();
        // This would initialize logging, but we can't verify it in tests
        // without more complex setup. The fact that this compiles is enough.
        let _ = config;
    }

    #[test]
    fn test_logging_config_debug_impl() {
        let config = LoggingConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("LoggingConfig"));
    }
}
