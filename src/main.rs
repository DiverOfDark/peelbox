use aipack::cli::commands::{CliArgs, Commands, DetectArgs, HealthArgs};
use aipack::cli::output::{EnvVarInfo, HealthStatus, OutputFormat, OutputFormatter};
use aipack::config::AipackConfig;
use aipack::detection::service::DetectionService;
use aipack::llm::{select_llm_client, RecordingLLMClient, RecordingMode};
use aipack::progress::{LoggingHandler, ProgressHandler};
use aipack::VERSION;
use genai::adapter::AdapterKind;

use clap::Parser;
use std::collections::HashMap;
use std::env;
use std::process;
use std::sync::Arc;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    init_logging_from_args(&args);

    debug!("aipack v{} starting", VERSION);
    debug!("Arguments: {:?}", args);

    let exit_code = match &args.command {
        Commands::Detect(detect_args) => handle_detect(detect_args, args.quiet, args.verbose).await,
        Commands::Health(health_args) => handle_health(health_args).await,
    };

    process::exit(exit_code);
}

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
            let level_str = env::var("AIPACK_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
            parse_level(&level_str)
        };

        let mut filter = EnvFilter::from_default_env();

        if env::var("RUST_LOG").is_err() {
            filter = filter
                .add_directive(format!("aipack={}", level).parse().unwrap())
                .add_directive("h2=warn".parse().unwrap())
                .add_directive("hyper=warn".parse().unwrap())
                .add_directive("reqwest=warn".parse().unwrap());
        }

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true).with_writer(std::io::stderr))
            .init();
    });
}

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

async fn handle_detect(args: &DetectArgs, quiet: bool, verbose: bool) -> i32 {
    info!("Starting build system detection");

    let repo_path = args
        .repository_path
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    debug!("Repository path: {}", repo_path.display());

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

    let repo_path = match repo_path.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to canonicalize repository path: {}", e);
            eprintln!("Error: Failed to canonicalize repository path: {}", e);
            return 1;
        }
    };
    debug!("Canonicalized repository path: {}", repo_path.display());

    let default_config = AipackConfig::default();
    let config = AipackConfig {
        provider: args.backend.unwrap_or(default_config.provider),
        model: args.model.clone().unwrap_or(default_config.model),
        request_timeout_secs: args.timeout,
        cache_enabled: !args.no_cache && default_config.cache_enabled,
        ..default_config
    };
    if args.backend.is_some() {
        debug!("Provider explicitly set to: {:?}", config.provider);
    }
    if args.model.is_some() {
        debug!("Model overridden to: {}", config.model);
    }
    if args.no_cache {
        debug!("Caching disabled");
    }

    if let Err(e) = config.validate() {
        error!("Configuration error: {}", e);
        eprintln!("Configuration error: {}", e);
        eprintln!("\nPlease check your environment variables and command-line arguments.");
        return 1;
    }

    let wrap_with_recording =
        |client: Arc<dyn aipack::llm::LLMClient>| -> Arc<dyn aipack::llm::LLMClient> {
            if std::env::var("AIPACK_ENABLE_RECORDING").is_ok() {
                let recordings_dir = std::path::PathBuf::from("tests/recordings");
                match RecordingLLMClient::new(client.clone(), RecordingMode::Auto, recordings_dir) {
                    Ok(recording_client) => {
                        debug!("Recording enabled, using tests/recordings/ directory");
                        return Arc::new(recording_client) as Arc<dyn aipack::llm::LLMClient>;
                    }
                    Err(e) => {
                        warn!(
                            "Failed to enable recording: {}. Continuing without recording.",
                            e
                        );
                    }
                }
            }
            client
        };

    info!("Initializing detection service");
    let service = if args.backend.is_some() {
        debug!("Using explicitly specified backend: {:?}", config.provider);

        use aipack::llm::GenAIClient;
        use std::time::Duration;

        let client = match GenAIClient::new(
            config.provider,
            config.model.clone(),
            Duration::from_secs(config.request_timeout_secs),
        )
        .await
        {
            Ok(c) => wrap_with_recording(Arc::new(c) as Arc<dyn aipack::llm::LLMClient>),
            Err(e) => {
                error!("Failed to initialize backend: {}", e);
                eprintln!("Failed to initialize backend: {}", e);
                eprintln!("\nPossible solutions:");
                match config.provider {
                    AdapterKind::Ollama => {
                        eprintln!("  - Ensure Ollama is running: ollama serve");
                        eprintln!("  - Check OLLAMA_HOST environment variable (default: http://localhost:11434)");
                        eprintln!(
                            "  - Try a different provider: --backend openai, --backend claude, etc."
                        );
                    }
                    AdapterKind::OpenAI => {
                        eprintln!("  - Set OPENAI_API_KEY environment variable");
                        eprintln!("  - Optionally set OPENAI_API_BASE for custom endpoints (e.g., Azure OpenAI)");
                    }
                    AdapterKind::Anthropic => {
                        eprintln!("  - Set ANTHROPIC_API_KEY environment variable");
                    }
                    AdapterKind::Gemini => {
                        eprintln!("  - Set GOOGLE_API_KEY environment variable");
                    }
                    AdapterKind::Xai => {
                        eprintln!("  - Set XAI_API_KEY environment variable");
                    }
                    AdapterKind::Groq => {
                        eprintln!("  - Set GROQ_API_KEY environment variable");
                    }
                    _ => {
                        eprintln!("  - Check provider-specific environment variables");
                        eprintln!("  - Refer to provider documentation for setup instructions");
                    }
                }
                eprintln!("  - Run 'aipack health' to check backend availability");
                eprintln!("  - Or omit --backend to automatically select an available backend");
                return 1;
            }
        };

        DetectionService::new(client)
    } else {
        info!("Auto-selecting best available backend");
        let interactive = atty::is(atty::Stream::Stdout);

        match select_llm_client(&config, interactive).await {
            Ok(selected) => {
                info!("Auto-selected backend: {}", selected.description);
                if !quiet {
                    eprintln!("Using: {}", selected.description);
                }

                let client = wrap_with_recording(selected.client);

                DetectionService::new(client)
            }
            Err(e) => {
                error!("Failed to auto-select backend: {}", e);
                eprintln!("Failed to auto-select backend: {}", e);
                eprintln!("\nNo LLM backend available. Please either:");
                eprintln!("  - Set an API key (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)");
                eprintln!("  - Start Ollama locally (ollama serve)");
                eprintln!("  - Ensure sufficient RAM for embedded LLM (minimum 3GB available)");
                return 1;
            }
        }
    };

    info!(
        "Using backend: {} ({})",
        service.backend_name(),
        service
            .backend_model_info()
            .unwrap_or_else(|| "default".to_string())
    );

    info!("Analyzing repository: {}", repo_path.display());

    let progress: Option<Arc<dyn ProgressHandler>> = if verbose {
        Some(Arc::new(LoggingHandler))
    } else {
        None
    };

    let results = match service
        .detect_with_progress(repo_path.clone(), progress)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Detection failed: {}", e);
            eprintln!("Detection failed: {}", e);
            return 1;
        }
    };

    info!("Detection complete: {} projects detected", results.len());

    let format: OutputFormat = args.format.into();
    let formatter = OutputFormatter::new(format);

    let output = match formatter.format_multiple(&results) {
        Ok(out) => out,
        Err(e) => {
            error!("Failed to format output: {}", e);
            eprintln!("Error: Failed to format output: {}", e);
            return 1;
        }
    };

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

    let lowest_confidence = results
        .iter()
        .map(|r| r.metadata.confidence)
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(1.0);

    if lowest_confidence < 0.7 {
        warn!(
            "Detection confidence is low ({:.1}%)",
            lowest_confidence * 100.0
        );
        2
    } else {
        0
    }
}

fn mask_api_key(value: &str) -> String {
    if value.len() <= 8 {
        "*".repeat(value.len())
    } else {
        format!("{}...{}", &value[..4], &value[value.len() - 4..])
    }
}

fn collect_env_var_info() -> HashMap<String, Vec<EnvVarInfo>> {
    let mut env_vars = HashMap::new();

    let ollama_host = env::var("OLLAMA_HOST");
    env_vars.insert(
        "Ollama".to_string(),
        vec![EnvVarInfo {
            name: "OLLAMA_HOST".to_string(),
            value: Some(
                ollama_host
                    .clone()
                    .unwrap_or_else(|_| "http://localhost:11434 (default)".to_string()),
            ),
            default: Some("http://localhost:11434".to_string()),
            required: false,
            description: "Ollama server endpoint".to_string(),
        }],
    );

    let openai_key = env::var("OPENAI_API_KEY");
    let openai_base = env::var("OPENAI_API_BASE");
    env_vars.insert(
        "OpenAI".to_string(),
        vec![
            EnvVarInfo {
                name: "OPENAI_API_KEY".to_string(),
                value: openai_key.ok().map(|k| mask_api_key(&k)),
                default: None,
                required: true,
                description: "OpenAI API key for authentication".to_string(),
            },
            EnvVarInfo {
                name: "OPENAI_API_BASE".to_string(),
                value: Some(
                    openai_base
                        .clone()
                        .unwrap_or_else(|_| "https://api.openai.com/v1 (default)".to_string()),
                ),
                default: Some("https://api.openai.com/v1".to_string()),
                required: false,
                description: "Custom API endpoint (e.g., for Azure OpenAI)".to_string(),
            },
        ],
    );

    let anthropic_key = env::var("ANTHROPIC_API_KEY");
    env_vars.insert(
        "Claude".to_string(),
        vec![EnvVarInfo {
            name: "ANTHROPIC_API_KEY".to_string(),
            value: anthropic_key.ok().map(|k| mask_api_key(&k)),
            default: None,
            required: true,
            description: "Anthropic API key for Claude access".to_string(),
        }],
    );

    let google_key = env::var("GOOGLE_API_KEY");
    env_vars.insert(
        "Gemini".to_string(),
        vec![EnvVarInfo {
            name: "GOOGLE_API_KEY".to_string(),
            value: google_key.ok().map(|k| mask_api_key(&k)),
            default: None,
            required: true,
            description: "Google AI API key for Gemini access".to_string(),
        }],
    );

    let xai_key = env::var("XAI_API_KEY");
    env_vars.insert(
        "Grok".to_string(),
        vec![EnvVarInfo {
            name: "XAI_API_KEY".to_string(),
            value: xai_key.ok().map(|k| mask_api_key(&k)),
            default: None,
            required: true,
            description: "xAI API key for Grok access".to_string(),
        }],
    );

    let groq_key = env::var("GROQ_API_KEY");
    env_vars.insert(
        "Groq".to_string(),
        vec![EnvVarInfo {
            name: "GROQ_API_KEY".to_string(),
            value: groq_key.ok().map(|k| mask_api_key(&k)),
            default: None,
            required: true,
            description: "Groq API key for fast inference".to_string(),
        }],
    );

    env_vars
}

async fn handle_health(args: &HealthArgs) -> i32 {
    info!("Checking backend health");

    let config = AipackConfig::default();
    let mut health_results = HashMap::new();

    let providers_to_check: Vec<AdapterKind> = if let Some(provider) = args.backend {
        vec![provider]
    } else {
        vec![
            AdapterKind::Ollama,
            AdapterKind::OpenAI,
            AdapterKind::Anthropic,
            AdapterKind::Gemini,
            AdapterKind::Xai,
            AdapterKind::Groq,
        ]
    };

    for provider in providers_to_check {
        let provider_name = format!("{:?}", provider);
        debug!("Checking {} provider", provider_name);

        let status = match provider {
            AdapterKind::Ollama => {
                let ollama_host = env::var("OLLAMA_HOST")
                    .unwrap_or_else(|_| "http://localhost:11434".to_string());
                let url = format!("{}/api/tags", ollama_host);
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(2))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new());

                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => {
                        info!("Ollama is available at {}", ollama_host);
                        HealthStatus::available(format!("Connected to {}", ollama_host))
                            .with_details(format!("Model: {}", config.model))
                    }
                    _ => {
                        warn!("Ollama is not available at {}", ollama_host);
                        HealthStatus::unavailable(format!("Cannot connect to {}", ollama_host))
                            .with_details("Ensure Ollama is running: ollama serve".to_string())
                    }
                }
            }
            AdapterKind::OpenAI => match env::var("OPENAI_API_KEY") {
                Ok(_) => {
                    info!("OpenAI API key is configured");
                    HealthStatus::available("API key is configured".to_string())
                }
                Err(_) => {
                    warn!("OpenAI API key is not configured");
                    HealthStatus::unavailable("API key not configured".to_string())
                        .with_details("Set OPENAI_API_KEY environment variable".to_string())
                }
            },
            AdapterKind::Anthropic => match env::var("ANTHROPIC_API_KEY") {
                Ok(_) => {
                    info!("Anthropic API key is configured");
                    HealthStatus::available("API key is configured".to_string())
                }
                Err(_) => {
                    warn!("Anthropic API key is not configured");
                    HealthStatus::unavailable("API key not configured".to_string())
                        .with_details("Set ANTHROPIC_API_KEY environment variable".to_string())
                }
            },
            AdapterKind::Gemini => match env::var("GOOGLE_API_KEY") {
                Ok(_) => {
                    info!("Google API key is configured");
                    HealthStatus::available("API key is configured".to_string())
                }
                Err(_) => {
                    warn!("Google API key is not configured");
                    HealthStatus::unavailable("API key not configured".to_string())
                        .with_details("Set GOOGLE_API_KEY environment variable".to_string())
                }
            },
            AdapterKind::Xai => match env::var("XAI_API_KEY") {
                Ok(_) => {
                    info!("xAI API key is configured");
                    HealthStatus::available("API key is configured".to_string())
                }
                Err(_) => {
                    warn!("xAI API key is not configured");
                    HealthStatus::unavailable("API key not configured".to_string())
                        .with_details("Set XAI_API_KEY environment variable".to_string())
                }
            },
            AdapterKind::Groq => match env::var("GROQ_API_KEY") {
                Ok(_) => {
                    info!("Groq API key is configured");
                    HealthStatus::available("API key is configured".to_string())
                }
                Err(_) => {
                    warn!("Groq API key is not configured");
                    HealthStatus::unavailable("API key not configured".to_string())
                        .with_details("Set GROQ_API_KEY environment variable".to_string())
                }
            },
            _ => HealthStatus::unavailable(format!(
                "Provider {:?} is not supported by aipack",
                provider
            )),
        };

        health_results.insert(provider_name, status);
    }

    let env_vars = collect_env_var_info();

    let format: OutputFormat = args.format.into();
    let formatter = OutputFormatter::new(format);

    let output = match formatter.format_health_with_env_vars(&health_results, &env_vars) {
        Ok(out) => out,
        Err(e) => {
            error!("Failed to format health output: {}", e);
            eprintln!("Error: Failed to format health output: {}", e);
            return 1;
        }
    };

    println!("{}", output);

    let all_available = health_results.values().all(|status| status.available);
    if all_available {
        0
    } else {
        1
    }
}
