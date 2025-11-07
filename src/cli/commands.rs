//! CLI command definitions using clap derive macros
//!
//! This module defines all CLI commands, arguments, and options for the aipack tool.
//! It uses clap's derive API for automatic help generation and argument parsing.
//!
//! # Commands
//!
//! - `detect` - Detect build commands in a repository
//! - `health` - Check backend availability
//! - `config` - Show current configuration
//!
//! # Example
//!
//! ```ignore
//! use aipack::cli::CliArgs;
//! use clap::Parser;
//!
//! let args = CliArgs::parse();
//! println!("Command: {:?}", args.command);
//! ```

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// AI-powered buildkit frontend for intelligent build command detection
#[derive(Parser, Debug)]
#[command(
    name = "aipack",
    about = "AI-powered buildkit frontend for intelligent build command detection",
    version,
    author,
    long_about = "aipack analyzes repository structure using LLMs to detect build systems \
                  and generate appropriate build commands. It supports multiple AI backends \
                  (Ollama, Mistral) and output formats."
)]
pub struct CliArgs {
    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,

    /// Logging level (error, warn, info, debug, trace)
    #[arg(long, global = true, value_name = "LEVEL", help = "Set logging level")]
    pub log_level: Option<String>,

    /// Enable verbose output (equivalent to --log-level debug)
    #[arg(
        short = 'v',
        long,
        global = true,
        help = "Increase verbosity (can be used multiple times)"
    )]
    pub verbose: bool,

    /// Suppress all output except errors
    #[arg(
        short = 'q',
        long,
        global = true,
        conflicts_with = "verbose",
        help = "Quiet mode - suppress non-error output"
    )]
    pub quiet: bool,
}

/// Available subcommands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Detect build commands in a repository
    #[command(
        about = "Detect build commands in a repository",
        long_about = "Analyzes repository structure and configuration files to detect the \
                      build system and generate appropriate build, test, and deploy commands.\n\n\
                      Examples:\n  \
                      aipack detect\n  \
                      aipack detect /path/to/repo\n  \
                      aipack detect --format json\n  \
                      aipack detect --backend ollama --model qwen:14b"
    )]
    Detect(DetectArgs),

    /// Check backend availability and health
    #[command(
        about = "Check backend availability",
        long_about = "Checks the availability and health of configured AI backends.\n\n\
                      Examples:\n  \
                      aipack health\n  \
                      aipack health --backend ollama"
    )]
    Health(HealthArgs),

    /// Show current configuration
    #[command(
        about = "Show current configuration",
        long_about = "Displays the current aipack configuration including backend settings, \
                      timeouts, and other options. API keys are masked by default.\n\n\
                      Examples:\n  \
                      aipack config\n  \
                      aipack config --show-secrets"
    )]
    Config(ConfigArgs),
}

/// Arguments for the detect command
#[derive(Parser, Debug, Clone)]
pub struct DetectArgs {
    /// Path to repository to analyze (default: current directory)
    #[arg(
        value_name = "PATH",
        help = "Path to repository (defaults to current directory)"
    )]
    pub repository_path: Option<PathBuf>,

    /// Output format
    #[arg(
        short = 'f',
        long,
        value_enum,
        default_value = "human",
        help = "Output format"
    )]
    pub format: OutputFormatArg,

    /// Force specific backend
    #[arg(
        short = 'b',
        long,
        value_enum,
        default_value = "auto",
        help = "AI backend to use for detection"
    )]
    pub backend: BackendArg,

    /// Model name (for Ollama backend)
    #[arg(
        short = 'm',
        long,
        value_name = "MODEL",
        help = "Model name to use (Ollama only, e.g., 'qwen:14b')"
    )]
    pub model: Option<String>,

    /// Request timeout in seconds
    #[arg(
        long,
        value_name = "SECONDS",
        default_value = "60",
        help = "Request timeout in seconds"
    )]
    pub timeout: u64,

    /// Include raw file contents in output
    #[arg(long, help = "Include raw file contents in verbose output")]
    pub verbose_output: bool,

    /// Disable result caching
    #[arg(long, help = "Disable result caching")]
    pub no_cache: bool,

    /// Write output to file instead of stdout
    #[arg(
        short = 'o',
        long,
        value_name = "FILE",
        help = "Write output to file instead of stdout"
    )]
    pub output: Option<PathBuf>,
}

/// Arguments for the health command
#[derive(Parser, Debug, Clone)]
pub struct HealthArgs {
    /// Backend to check (omit to check all)
    #[arg(
        short = 'b',
        long,
        value_enum,
        help = "Specific backend to check (omit to check all)"
    )]
    pub backend: Option<BackendArg>,

    /// Output format
    #[arg(
        short = 'f',
        long,
        value_enum,
        default_value = "human",
        help = "Output format"
    )]
    pub format: OutputFormatArg,
}

/// Arguments for the config command
#[derive(Parser, Debug, Clone)]
pub struct ConfigArgs {
    /// Show secrets (API keys) unmasked
    #[arg(long, help = "Show API keys and secrets (unmasked)")]
    pub show_secrets: bool,

    /// Output format
    #[arg(
        short = 'f',
        long,
        value_enum,
        default_value = "human",
        help = "Output format"
    )]
    pub format: OutputFormatArg,
}

/// Output format argument enum
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormatArg {
    /// JSON format
    Json,
    /// YAML format
    Yaml,
    /// Human-readable format
    Human,
}

impl From<OutputFormatArg> for super::output::OutputFormat {
    fn from(arg: OutputFormatArg) -> Self {
        match arg {
            OutputFormatArg::Json => super::output::OutputFormat::Json,
            OutputFormatArg::Yaml => super::output::OutputFormat::Yaml,
            OutputFormatArg::Human => super::output::OutputFormat::Human,
        }
    }
}

/// Backend selection argument enum
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendArg {
    /// Automatically select best available backend
    Auto,
    /// Use Ollama (local) backend
    Ollama,
    /// Use Mistral API backend
    Mistral,
}

impl std::fmt::Display for BackendArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendArg::Auto => write!(f, "auto"),
            BackendArg::Ollama => write!(f, "ollama"),
            BackendArg::Mistral => write!(f, "mistral"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_args_verify() {
        // Verify that CLI structure is valid
        CliArgs::command().debug_assert();
    }

    #[test]
    fn test_default_detect_args() {
        let args = CliArgs::parse_from(&["aipack", "detect"]);
        match args.command {
            Commands::Detect(detect_args) => {
                assert_eq!(detect_args.format, OutputFormatArg::Human);
                assert_eq!(detect_args.backend, BackendArg::Auto);
                assert_eq!(detect_args.timeout, 60);
                assert!(!detect_args.verbose_output);
                assert!(!detect_args.no_cache);
                assert!(detect_args.repository_path.is_none());
            }
            _ => panic!("Expected Detect command"),
        }
    }

    #[test]
    fn test_detect_with_path() {
        let args = CliArgs::parse_from(&["aipack", "detect", "/tmp/repo"]);
        match args.command {
            Commands::Detect(detect_args) => {
                assert_eq!(
                    detect_args.repository_path,
                    Some(PathBuf::from("/tmp/repo"))
                );
            }
            _ => panic!("Expected Detect command"),
        }
    }

    #[test]
    fn test_detect_with_options() {
        let args = CliArgs::parse_from(&[
            "aipack",
            "detect",
            "--format",
            "json",
            "--backend",
            "ollama",
            "--model",
            "qwen:14b",
            "--timeout",
            "120",
            "--verbose-output",
            "--no-cache",
        ]);

        match args.command {
            Commands::Detect(detect_args) => {
                assert_eq!(detect_args.format, OutputFormatArg::Json);
                assert_eq!(detect_args.backend, BackendArg::Ollama);
                assert_eq!(detect_args.model, Some("qwen:14b".to_string()));
                assert_eq!(detect_args.timeout, 120);
                assert!(detect_args.verbose_output);
                assert!(detect_args.no_cache);
            }
            _ => panic!("Expected Detect command"),
        }
    }

    #[test]
    fn test_health_command() {
        let args = CliArgs::parse_from(&["aipack", "health"]);
        match args.command {
            Commands::Health(health_args) => {
                assert!(health_args.backend.is_none());
                assert_eq!(health_args.format, OutputFormatArg::Human);
            }
            _ => panic!("Expected Health command"),
        }
    }

    #[test]
    fn test_health_with_backend() {
        let args = CliArgs::parse_from(&["aipack", "health", "--backend", "ollama"]);
        match args.command {
            Commands::Health(health_args) => {
                assert_eq!(health_args.backend, Some(BackendArg::Ollama));
            }
            _ => panic!("Expected Health command"),
        }
    }

    #[test]
    fn test_config_command() {
        let args = CliArgs::parse_from(&["aipack", "config"]);
        match args.command {
            Commands::Config(config_args) => {
                assert!(!config_args.show_secrets);
                assert_eq!(config_args.format, OutputFormatArg::Human);
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_config_show_secrets() {
        let args = CliArgs::parse_from(&["aipack", "config", "--show-secrets"]);
        match args.command {
            Commands::Config(config_args) => {
                assert!(config_args.show_secrets);
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_global_verbose_flag() {
        let args = CliArgs::parse_from(&["aipack", "-v", "detect"]);
        assert!(args.verbose);
        assert!(!args.quiet);
    }

    #[test]
    fn test_global_quiet_flag() {
        let args = CliArgs::parse_from(&["aipack", "-q", "detect"]);
        assert!(!args.verbose);
        assert!(args.quiet);
    }

    #[test]
    fn test_log_level_flag() {
        let args = CliArgs::parse_from(&["aipack", "--log-level", "debug", "detect"]);
        assert_eq!(args.log_level, Some("debug".to_string()));
    }

    #[test]
    fn test_backend_arg_display() {
        assert_eq!(BackendArg::Auto.to_string(), "auto");
        assert_eq!(BackendArg::Ollama.to_string(), "ollama");
        assert_eq!(BackendArg::Mistral.to_string(), "mistral");
    }
}
