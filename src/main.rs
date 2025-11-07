//! Main CLI entry point for aipack
//!
//! This binary provides the command-line interface for aipack, orchestrating
//! repository detection, backend health checks, and configuration management.
//!
//! # Commands
//!
//! - `detect` - Detect build commands in a repository
//! - `health` - Check backend availability
//! - `config` - Show current configuration
//!
//! # Example Usage
//!
//! ```bash
//! # Detect build system in current directory
//! aipack detect
//!
//! # Detect with JSON output
//! aipack detect --format json
//!
//! # Check backend health
//! aipack health
//!
//! # Show configuration
//! aipack config
//! ```

use aipack::cli::commands::{BackendArg, CliArgs, Commands, ConfigArgs, DetectArgs, HealthArgs};
use aipack::cli::output::{HealthStatus, OutputFormat, OutputFormatter};
use aipack::config::AipackConfig;
use aipack::detection::analyzer::RepositoryAnalyzer;
use aipack::detection::service::DetectionService;
use aipack::util::logging::{init_logging, parse_level, LoggingConfig};
use aipack::VERSION;

use clap::Parser;
use std::collections::HashMap;
use std::env;
use std::process;
use tracing::{debug, error, info, warn};

#[tokio::main]
async fn main() {
    // Parse CLI arguments
    let args = CliArgs::parse();

    // Initialize logging based on CLI flags
    init_logging_from_args(&args);

    // Log startup
    debug!("aipack v{} starting", VERSION);
    debug!("Arguments: {:?}", args);

    // Execute the appropriate command
    let exit_code = match &args.command {
        Commands::Detect(detect_args) => handle_detect(detect_args, args.quiet).await,
        Commands::Health(health_args) => handle_health(health_args).await,
        Commands::Config(config_args) => handle_config(config_args),
    };

    // Exit with appropriate code
    process::exit(exit_code);
}

/// Initializes logging based on CLI arguments
///
/// Respects the following priority (highest to lowest):
/// 1. --log-level flag
/// 2. --verbose flag
/// 3. --quiet flag
/// 4. AIPACK_LOG_LEVEL environment variable
/// 5. Default (INFO)
fn init_logging_from_args(args: &CliArgs) {
    let level = if let Some(level_str) = &args.log_level {
        parse_level(level_str)
    } else if args.verbose {
        tracing::Level::DEBUG
    } else if args.quiet {
        tracing::Level::ERROR
    } else {
        // Fall back to environment or default
        let level_str = env::var("AIPACK_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        parse_level(&level_str)
    };

    let config = LoggingConfig::with_level(level);
    init_logging(config);
}

/// Handles the detect command
///
/// This function:
/// 1. Loads configuration
/// 2. Validates repository path
/// 3. Creates detection service
/// 4. Performs detection
/// 5. Formats and outputs results
async fn handle_detect(args: &DetectArgs, quiet: bool) -> i32 {
    info!("Starting build system detection");

    // Determine repository path (default to current directory)
    let repo_path = args
        .repository_path
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    debug!("Repository path: {}", repo_path.display());

    // Validate repository path exists
    if !repo_path.exists() {
        error!("Repository path does not exist: {}", repo_path.display());
        eprintln!(
            "Error: Repository path does not exist: {}",
            repo_path.display()
        );
        return 1;
    }

    if !repo_path.is_dir() {
        error!(
            "Repository path is not a directory: {}",
            repo_path.display()
        );
        eprintln!(
            "Error: Repository path is not a directory: {}",
            repo_path.display()
        );
        return 1;
    }

    // Load configuration
    let mut config = AipackConfig::default();

    // Override backend if specified
    if args.backend != BackendArg::Auto {
        config.backend = args.backend.to_string();
        debug!("Backend overridden to: {}", config.backend);
    }

    // Override model if specified (Ollama only)
    if let Some(model) = &args.model {
        if config.backend == "ollama" || args.backend == BackendArg::Ollama {
            config.ollama_model = model.clone();
            debug!("Ollama model overridden to: {}", model);
        } else {
            warn!("--model flag is only applicable for Ollama backend");
        }
    }

    // Override timeout
    config.request_timeout_secs = args.timeout;

    // Disable caching if requested
    if args.no_cache {
        config.cache_enabled = false;
        debug!("Caching disabled");
    }

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration error: {}", e);
        eprintln!("Configuration error: {}", e);
        eprintln!("\nPlease check your environment variables and command-line arguments.");
        eprintln!("Run 'aipack config' to see current configuration.");
        return 1;
    }

    // Create detection service
    info!("Initializing detection service");
    let service = match DetectionService::new(&config).await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to initialize detection service: {}", e);
            eprintln!("Failed to initialize detection service: {}", e);
            eprintln!("\nPossible solutions:");
            match e {
                aipack::detection::service::ServiceError::BackendInitError(ref msg) => {
                    if msg.contains("Ollama") {
                        eprintln!("  - Ensure Ollama is running: ollama serve");
                        eprintln!("  - Check Ollama endpoint: {}", config.ollama_endpoint);
                        eprintln!("  - Try using Mistral backend: --backend mistral");
                    } else if msg.contains("Mistral") {
                        eprintln!("  - Set MISTRAL_API_KEY environment variable");
                        eprintln!("  - Try using Ollama backend: --backend ollama");
                    }
                }
                _ => {
                    eprintln!("  - Run 'aipack health' to check backend availability");
                    eprintln!("  - Run 'aipack config' to verify configuration");
                }
            }
            return 1;
        }
    };

    info!(
        "Using backend: {} ({})",
        service.backend_name(),
        service
            .backend_model_info()
            .unwrap_or_else(|| "default".to_string())
    );

    // Perform detection
    info!("Analyzing repository: {}", repo_path.display());
    let result = match service.detect(repo_path.clone()).await {
        Ok(r) => r,
        Err(e) => {
            error!("Detection failed: {}", e);
            eprintln!("Detection failed: {}", e);
            return 1;
        }
    };

    info!(
        "Detection complete: {} ({}) with {:.1}% confidence",
        result.build_system,
        result.language,
        result.confidence * 100.0
    );

    // Format output
    let format: OutputFormat = args.format.into();
    let formatter = OutputFormatter::new(format);

    let output = if args.verbose_output {
        // Include repository context in verbose mode
        let analyzer = RepositoryAnalyzer::new(repo_path.clone());
        match analyzer.analyze().await {
            Ok(context) => match formatter.format_with_context(&result, &context) {
                Ok(out) => out,
                Err(e) => {
                    error!("Failed to format output: {}", e);
                    eprintln!("Error: Failed to format output: {}", e);
                    return 1;
                }
            },
            Err(e) => {
                error!("Failed to analyze repository for verbose output: {}", e);
                eprintln!("Warning: Failed to gather verbose context: {}", e);
                eprintln!("Falling back to regular output format");
                // Fall back to regular output
                match formatter.format(&result) {
                    Ok(out) => out,
                    Err(e) => {
                        error!("Failed to format output: {}", e);
                        eprintln!("Error: Failed to format output: {}", e);
                        return 1;
                    }
                }
            }
        }
    } else {
        match formatter.format(&result) {
            Ok(out) => out,
            Err(e) => {
                error!("Failed to format output: {}", e);
                eprintln!("Error: Failed to format output: {}", e);
                return 1;
            }
        }
    };

    // Write output to file or stdout
    if let Some(output_file) = &args.output {
        match std::fs::write(output_file, &output) {
            Ok(_) => {
                info!("Output written to: {}", output_file.display());
                if !quiet {
                    println!("Output written to: {}", output_file.display());
                }
            }
            Err(e) => {
                error!("Failed to write output to file: {}", e);
                eprintln!(
                    "Error: Failed to write output to {}: {}",
                    output_file.display(),
                    e
                );
                return 1;
            }
        }
    } else {
        println!("{}", output);
    }

    // Exit with warning code if confidence is low
    if result.is_low_confidence() {
        warn!(
            "Detection confidence is low ({:.1}%)",
            result.confidence * 100.0
        );
        2 // Exit code 2 for low confidence
    } else {
        0 // Success
    }
}

/// Handles the health command
///
/// Checks availability of configured backends and displays status
async fn handle_health(args: &HealthArgs) -> i32 {
    info!("Checking backend health");

    let config = AipackConfig::default();

    let mut health_results = HashMap::new();

    // Determine which backends to check
    let check_all = args.backend.is_none();
    let check_ollama = check_all || args.backend == Some(BackendArg::Ollama);
    let check_lm_studio = check_all || args.backend == Some(BackendArg::LMStudio);
    let check_mistral = check_all || args.backend == Some(BackendArg::Mistral);

    // Check Ollama
    if check_ollama {
        debug!("Checking Ollama at {}", config.ollama_endpoint);

        // Check availability using async reqwest client
        let url = format!("{}/api/tags", config.ollama_endpoint);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let status = match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                info!("Ollama is available at {}", config.ollama_endpoint);
                HealthStatus::available(format!("Connected to {}", config.ollama_endpoint))
                    .with_details(format!("Model: {}", config.ollama_model))
            }
            _ => {
                warn!("Ollama is not available at {}", config.ollama_endpoint);
                HealthStatus::unavailable(format!("Cannot connect to {}", config.ollama_endpoint))
                    .with_details("Ensure Ollama is running: ollama serve".to_string())
            }
        };
        health_results.insert("Ollama".to_string(), status);
    }

    // Check LM Studio
    if check_lm_studio {
        debug!("Checking LM Studio at {}", config.lm_studio_endpoint);

        // Check availability using async reqwest client
        let url = format!("{}/v1/models", config.lm_studio_endpoint);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let status = match client.get(&url).send().await {
            Ok(response) if response.status().is_success() => {
                info!("LM Studio is available at {}", config.lm_studio_endpoint);
                HealthStatus::available(format!("Connected to {}", config.lm_studio_endpoint))
            }
            _ => {
                warn!(
                    "LM Studio is not available at {}",
                    config.lm_studio_endpoint
                );
                HealthStatus::unavailable(format!(
                    "Cannot connect to {}",
                    config.lm_studio_endpoint
                ))
                .with_details("Ensure LM Studio is running on the configured endpoint".to_string())
            }
        };
        health_results.insert("LM Studio".to_string(), status);
    }

    // Check Mistral
    if check_mistral {
        debug!("Checking Mistral API configuration");
        let status = if config.has_mistral_key() {
            info!("Mistral API key is configured");
            HealthStatus::available("API key is configured".to_string())
                .with_details(format!("Model: {}", config.mistral_model))
        } else {
            warn!("Mistral API key is not configured");
            HealthStatus::unavailable("API key not configured".to_string())
                .with_details("Set MISTRAL_API_KEY environment variable".to_string())
        };
        health_results.insert("Mistral".to_string(), status);
    }

    // Format and display results
    let format: OutputFormat = args.format.into();
    let formatter = OutputFormatter::new(format);

    let output = match formatter.format_health(&health_results) {
        Ok(out) => out,
        Err(e) => {
            error!("Failed to format health output: {}", e);
            eprintln!("Error: Failed to format health output: {}", e);
            return 1;
        }
    };

    println!("{}", output);

    // Return error code if any backend is unavailable
    let all_available = health_results.values().all(|status| status.available);
    if all_available {
        0
    } else {
        1
    }
}

/// Handles the config command
///
/// Displays current configuration
fn handle_config(args: &ConfigArgs) -> i32 {
    info!("Displaying configuration");

    let config = AipackConfig::default();

    // Format and display configuration
    let format: OutputFormat = args.format.into();
    let formatter = OutputFormatter::new(format);

    let output = match formatter.format_config(&config, args.show_secrets) {
        Ok(out) => out,
        Err(e) => {
            error!("Failed to format config output: {}", e);
            eprintln!("Error: Failed to format config output: {}", e);
            return 1;
        }
    };

    println!("{}", output);

    // Validate configuration and warn about issues
    if let Err(e) = config.validate() {
        warn!("Configuration validation failed: {}", e);
        eprintln!("\nWarning: Configuration has issues: {}", e);
        return 2;
    }

    0
}
