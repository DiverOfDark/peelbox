//! Main CLI entry point for aipack
//!
//! This binary provides the command-line interface for aipack, orchestrating
//! repository detection, backend health checks, and configuration management.
//!
//! # Commands
//!
//! - `detect` - Detect build commands in a repository
//! - `health` - Check backend availability
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
//! ```

use aipack::ai::genai_backend::Provider;
use aipack::cli::commands::{CliArgs, Commands, DetectArgs, HealthArgs};
use aipack::cli::output::{HealthStatus, OutputFormat, OutputFormatter};
use aipack::config::AipackConfig;
use aipack::detection::analyzer::RepositoryAnalyzer;
use aipack::detection::service::DetectionService;
use aipack::VERSION;

use clap::Parser;
use std::collections::HashMap;
use std::env;
use std::process;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let level = if let Some(level_str) = &args.log_level {
            parse_level(level_str)
        } else if args.verbose {
            Level::DEBUG
        } else if args.quiet {
            Level::ERROR
        } else {
            // Fall back to environment or default
            let level_str = env::var("AIPACK_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
            parse_level(&level_str)
        };

        // Build the EnvFilter
        let mut filter = EnvFilter::from_default_env()
            .add_directive(format!("aipack={}", level).parse().unwrap());

        // If RUST_LOG is not set, quiet down noisy dependencies
        if env::var("RUST_LOG").is_err() {
            filter = filter
                .add_directive("h2=warn".parse().unwrap())
                .add_directive("hyper=warn".parse().unwrap())
                .add_directive("reqwest=warn".parse().unwrap());
        }

        // Initialize tracing subscriber
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true))
            .init();
    });
}

/// Parses a log level from a string
fn parse_level(level_str: &str) -> Level {
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

    // Override provider if specified
    config.provider = args.backend;
    debug!("Provider set to: {:?}", config.provider);

    // Override model if specified (Ollama only)
    if let Some(model) = &args.model {
        if matches!(config.provider, Provider::Ollama) {
            config.ollama_model = model.clone();
            debug!("Ollama model overridden to: {}", model);
        } else {
            warn!("--model flag is only applicable for Ollama provider");
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
            match config.provider {
                Provider::Ollama => {
                    eprintln!("  - Ensure Ollama is running: ollama serve");
                    eprintln!("  - Check OLLAMA_HOST environment variable (default: http://localhost:11434)");
                    eprintln!("  - Try a different provider: --backend openai, --backend claude, etc.");
                }
                Provider::OpenAI => {
                    eprintln!("  - Set OPENAI_API_KEY environment variable");
                    eprintln!("  - Optionally set OPENAI_API_BASE for custom endpoints (e.g., Azure OpenAI)");
                }
                Provider::Claude => {
                    eprintln!("  - Set ANTHROPIC_API_KEY environment variable");
                }
                Provider::Gemini => {
                    eprintln!("  - Set GOOGLE_API_KEY environment variable");
                }
                Provider::Grok => {
                    eprintln!("  - Set XAI_API_KEY environment variable");
                }
                Provider::Groq => {
                    eprintln!("  - Set GROQ_API_KEY environment variable");
                }
            }
            eprintln!("  - Run 'aipack health' to check backend availability");
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
/// Checks availability of configured backends and displays status.
async fn handle_health(args: &HealthArgs) -> i32 {
    info!("Checking backend health");

    let config = AipackConfig::default();
    let mut health_results = HashMap::new();

    // Determine which providers to check
    let providers_to_check: Vec<Provider> = if let Some(provider) = args.backend {
        vec![provider]
    } else {
        // Check all supported providers
        vec![
            Provider::Ollama,
            Provider::OpenAI,
            Provider::Claude,
            Provider::Gemini,
            Provider::Grok,
            Provider::Groq,
        ]
    };

    for provider in providers_to_check {
        let provider_name = format!("{:?}", provider);
        debug!("Checking {} provider", provider_name);

        let status = match provider {
            Provider::Ollama => {
                // Check Ollama availability by attempting to connect
                let ollama_host = env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
                let url = format!("{}/api/tags", ollama_host);
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(2))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        info!("Ollama is available at {}", ollama_host);
                        HealthStatus::available(format!("Connected to {}", ollama_host))
                            .with_details(format!("Model: {}", config.ollama_model))
                    }
                    _ => {
                        warn!("Ollama is not available at {}", ollama_host);
                        HealthStatus::unavailable(format!("Cannot connect to {}", ollama_host))
                            .with_details("Ensure Ollama is running: ollama serve".to_string())
                    }
                }
            }
            Provider::OpenAI => {
                // Check if OpenAI API key is configured
                match env::var("OPENAI_API_KEY") {
                    Ok(_) => {
                        info!("OpenAI API key is configured");
                        HealthStatus::available("API key is configured".to_string())
                    }
                    Err(_) => {
                        warn!("OpenAI API key is not configured");
                        HealthStatus::unavailable("API key not configured".to_string())
                            .with_details("Set OPENAI_API_KEY environment variable".to_string())
                    }
                }
            }
            Provider::Claude => {
                // Check if Anthropic API key is configured
                match env::var("ANTHROPIC_API_KEY") {
                    Ok(_) => {
                        info!("Anthropic API key is configured");
                        HealthStatus::available("API key is configured".to_string())
                    }
                    Err(_) => {
                        warn!("Anthropic API key is not configured");
                        HealthStatus::unavailable("API key not configured".to_string())
                            .with_details("Set ANTHROPIC_API_KEY environment variable".to_string())
                    }
                }
            }
            Provider::Gemini => {
                // Check if Google API key is configured
                match env::var("GOOGLE_API_KEY") {
                    Ok(_) => {
                        info!("Google API key is configured");
                        HealthStatus::available("API key is configured".to_string())
                    }
                    Err(_) => {
                        warn!("Google API key is not configured");
                        HealthStatus::unavailable("API key not configured".to_string())
                            .with_details("Set GOOGLE_API_KEY environment variable".to_string())
                    }
                }
            }
            Provider::Grok => {
                // Check if xAI API key is configured
                match env::var("XAI_API_KEY") {
                    Ok(_) => {
                        info!("xAI API key is configured");
                        HealthStatus::available("API key is configured".to_string())
                    }
                    Err(_) => {
                        warn!("xAI API key is not configured");
                        HealthStatus::unavailable("API key not configured".to_string())
                            .with_details("Set XAI_API_KEY environment variable".to_string())
                    }
                }
            }
            Provider::Groq => {
                // Check if Groq API key is configured
                match env::var("GROQ_API_KEY") {
                    Ok(_) => {
                        info!("Groq API key is configured");
                        HealthStatus::available("API key is configured".to_string())
                    }
                    Err(_) => {
                        warn!("Groq API key is not configured");
                        HealthStatus::unavailable("API key not configured".to_string())
                            .with_details("Set GROQ_API_KEY environment variable".to_string())
                    }
                }
            }
        };

        health_results.insert(provider_name, status);
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

