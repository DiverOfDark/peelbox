use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Deterministic BuildKit frontend for intelligent build detection
#[derive(Parser, Debug)]
#[command(
    name = "peelbox",
    about = "Deterministic BuildKit frontend for intelligent build detection",
    version,
    author,
    long_about = "peelbox analyzes repository structure to detect build systems and generate \
                  Wolfi-based, distroless container build specifications. Detection is fully \
                  static and deterministic: no API keys, no network LLM calls, reproducible output."
)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(long, global = true, value_name = "LEVEL", help = "Set logging level")]
    pub log_level: Option<String>,

    #[arg(
        short = 'v',
        long,
        global = true,
        help = "Increase verbosity (can be used multiple times)"
    )]
    pub verbose: bool,

    #[arg(
        short = 'q',
        long,
        global = true,
        conflicts_with = "verbose",
        help = "Quiet mode - suppress non-error output"
    )]
    pub quiet: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(
        about = "Detect build commands in a repository",
        long_about = "Analyzes repository structure and configuration files to detect the \
                      build system and generate appropriate build, test, and deploy commands.\n\n\
                      Examples:\n  \
                      peelbox detect\n  \
                      peelbox detect /path/to/repo\n  \
                      peelbox detect --format json"
    )]
    Detect(DetectArgs),

    #[command(
        about = "Build image from UniversalBuild spec",
        long_about = "Build container image from UniversalBuild specification file.\n\
                      Connects to BuildKit daemon and executes build with real-time progress.\n\n\
                      Examples:\n  \
                      peelbox build --spec universalbuild.json --tag myapp:latest\n  \
                      peelbox build --spec spec.json --tag app:v1 --service api\n  \
                      peelbox build --spec spec.json --tag app:latest --context /path/to/project\n  \
                      peelbox build --spec spec.json --tag app:latest --output type=oci,dest=app.tar"
    )]
    Build(BuildArgs),
}

#[derive(Parser, Debug, Clone)]
pub struct DetectArgs {
    #[arg(
        value_name = "PATH",
        help = "Path to repository (defaults to current directory)"
    )]
    pub repository_path: Option<PathBuf>,

    #[arg(
        short = 'f',
        long,
        value_enum,
        default_value = "json",
        help = "Output format"
    )]
    pub format: OutputFormatArg,

    #[arg(
        short = 'o',
        long,
        value_name = "FILE",
        help = "Write output to file instead of stdout"
    )]
    pub output: Option<PathBuf>,
}

#[derive(Parser, Debug, Clone)]
pub struct BuildArgs {
    #[arg(
        long,
        value_name = "FILE",
        required = true,
        help = "Path to UniversalBuild JSON file"
    )]
    pub spec: PathBuf,

    #[arg(
        long,
        value_name = "TAG",
        required = true,
        help = "Image tag (e.g., myapp:latest)"
    )]
    pub tag: String,

    #[arg(
        long,
        value_name = "OUTPUT",
        help = "Output type (docker or oci,dest=file.tar, defaults to docker)"
    )]
    pub output: Option<String>,

    #[arg(
        long,
        value_name = "ADDR",
        help = "BuildKit daemon address (unix:///path, tcp://host:port, or docker-container://name)"
    )]
    pub buildkit: Option<String>,

    #[arg(
        long,
        value_name = "ENTRYPOINT",
        help = "Override entrypoint at build time"
    )]
    pub entrypoint: Option<String>,

    #[arg(
        long,
        value_name = "PLATFORM",
        help = "Target platform (e.g., linux/amd64)"
    )]
    pub platform: Option<String>,

    #[arg(
        long,
        value_name = "SERVICE",
        help = "Service name to build (required for monorepos with multiple services)"
    )]
    pub service: Option<String>,

    #[arg(
        long,
        value_name = "PATH",
        help = "Build context directory (defaults to current directory)"
    )]
    pub context: Option<PathBuf>,

    #[arg(
        long,
        default_value = "true",
        help = "Generate SBOM (Software Bill of Materials) attestation in SPDX format"
    )]
    pub sbom: bool,

    #[arg(long, help = "Disable SBOM attestation generation")]
    pub no_sbom: bool,

    #[arg(
        long,
        value_name = "MODE",
        help = "Generate SLSA provenance attestation (min or max, defaults to max)"
    )]
    pub provenance: Option<String>,

    #[arg(long, help = "Disable provenance attestation generation")]
    pub no_provenance: bool,

    #[arg(
        long,
        default_value = "true",
        help = "Scan build context files for SBOM generation"
    )]
    pub scan_context: bool,

    #[arg(
        long,
        value_name = "CACHE",
        help = "External cache configuration (e.g., user/app:cache, type=local,path=tmp/mydir). Used for both import and export."
    )]
    pub cache: Vec<String>,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormatArg {
    Json,
    Yaml,
}

impl From<OutputFormatArg> for super::output::OutputFormat {
    fn from(arg: OutputFormatArg) -> Self {
        match arg {
            OutputFormatArg::Json => super::output::OutputFormat::Json,
            OutputFormatArg::Yaml => super::output::OutputFormat::Yaml,
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
        let args = CliArgs::parse_from(["peelbox", "detect"]);
        match args.command {
            Commands::Detect(detect_args) => {
                assert_eq!(detect_args.format, OutputFormatArg::Json);
                assert!(detect_args.repository_path.is_none());
                assert!(detect_args.output.is_none());
            }
            _ => panic!("Expected Detect command"),
        }
    }

    #[test]
    fn test_detect_with_path() {
        let args = CliArgs::parse_from(["peelbox", "detect", "/tmp/repo"]);
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
        let args = CliArgs::parse_from([
            "peelbox", "detect", "--format", "yaml", "--output", "out.json",
        ]);

        match args.command {
            Commands::Detect(detect_args) => {
                assert_eq!(detect_args.format, OutputFormatArg::Yaml);
                assert_eq!(detect_args.output, Some(PathBuf::from("out.json")));
            }
            _ => panic!("Expected Detect command"),
        }
    }

    #[test]
    fn test_global_verbose_flag() {
        let args = CliArgs::parse_from(["peelbox", "-v", "detect"]);
        assert!(args.verbose);
        assert!(!args.quiet);
    }

    #[test]
    fn test_global_quiet_flag() {
        let args = CliArgs::parse_from(["peelbox", "-q", "detect"]);
        assert!(!args.verbose);
        assert!(args.quiet);
    }

    #[test]
    fn test_log_level_flag() {
        let args = CliArgs::parse_from(["peelbox", "--log-level", "debug", "detect"]);
        assert_eq!(args.log_level, Some("debug".to_string()));
    }
}
