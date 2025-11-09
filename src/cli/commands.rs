use crate::ai::genai_backend::Provider;
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
                  (Ollama, OpenAI, Claude, Gemini, Grok, Groq) and output formats."
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

    /// Check backend availability
    #[command(
        about = "Check backend availability",
        long_about = "Checks the availability and health of configured AI backends.\n\n\
                      Examples:\n  \
                      aipack health\n  \
                      aipack health --backend ollama"
    )]
    Health(HealthArgs),
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
        default_value = "ollama",
        help = "AI backend provider to use for detection"
    )]
    pub backend: Provider,

    /// Model name
    #[arg(
        short = 'm',
        long,
        value_name = "MODEL",
        help = "Model name to use (provider-specific, e.g., 'qwen:14b' for Ollama)"
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
    pub backend: Option<Provider>,

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
    /// Dockerfile format
    Dockerfile,
}

impl From<OutputFormatArg> for super::output::OutputFormat {
    fn from(arg: OutputFormatArg) -> Self {
        match arg {
            OutputFormatArg::Json => super::output::OutputFormat::Json,
            OutputFormatArg::Yaml => super::output::OutputFormat::Yaml,
            OutputFormatArg::Human => super::output::OutputFormat::Human,
            OutputFormatArg::Dockerfile => super::output::OutputFormat::Dockerfile,
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
                assert_eq!(detect_args.backend, Provider::Ollama);
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
                assert_eq!(detect_args.backend, Provider::Ollama);
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
                assert_eq!(health_args.backend, Some(Provider::Ollama));
            }
            _ => panic!("Expected Health command"),
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
    fn test_provider_display() {
        assert_eq!(Provider::Ollama.to_string(), "ollama");
        assert_eq!(Provider::OpenAI.to_string(), "openai");
        assert_eq!(Provider::Claude.to_string(), "claude");
        assert_eq!(Provider::Gemini.to_string(), "gemini");
        assert_eq!(Provider::Grok.to_string(), "grok");
        assert_eq!(Provider::Groq.to_string(), "groq");
    }
}
