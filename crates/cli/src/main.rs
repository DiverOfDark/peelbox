use genai::adapter::AdapterKind;
use peelbox_buildkit::filesend_service::OutputDestination;
use peelbox_buildkit::{
    progress::ProgressTracker, AttestationConfig, BuildKitConnection, BuildSession, CacheExport,
    CacheImport, ProvenanceMode,
};
use peelbox_cli::cli::commands::{BuildArgs, CliArgs, Commands, DetectArgs, HealthArgs};
use peelbox_cli::cli::output::{EnvVarInfo, HealthStatus, OutputFormat, OutputFormatter};
use peelbox_cli::{NAME, VERSION};
use peelbox_core::config::PeelboxConfig;
use peelbox_core::output::schema::UniversalBuild;
use peelbox_llm::{RecordingLLMClient, RecordingMode};
use peelbox_pipeline::detection::service::DetectionService;

use clap::Parser;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    init_logging_from_args(&args);

    debug!("{} v{} starting", NAME, VERSION);
    debug!("Arguments: {:?}", args);

    let exit_code = match &args.command {
        Commands::Detect(detect_args) => handle_detect(detect_args, args.quiet, args.verbose).await,
        Commands::Health(health_args) => handle_health(health_args).await,
        Commands::Build(build_args) => handle_build(build_args, args.quiet, args.verbose).await,
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
            let level_str = env::var("PEELBOX_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
            parse_level(&level_str)
        };

        let mut filter = EnvFilter::from_default_env();

        if env::var("RUST_LOG").is_err() {
            filter = filter
                .add_directive(format!("peelbox={}", level).parse().unwrap())
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

async fn handle_detect(args: &DetectArgs, quiet: bool, _verbose: bool) -> i32 {
    info!("Starting build system detection");

    let repo_path = args
        .repository_path
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    debug!("Repository path: {}", repo_path.display());

    if !repo_path.exists() {
        error!("Repository path does not exist: {}", repo_path.display());
        return 1;
    }

    if !repo_path.is_dir() {
        error!(
            "Repository path is not a directory: {}",
            repo_path.display()
        );
        return 1;
    }

    let repo_path: PathBuf = match repo_path.canonicalize() {
        Ok(path) => path,
        Err(e) => {
            error!("Failed to canonicalize repository path: {}", e);
            return 1;
        }
    };
    debug!("Canonicalized repository path: {}", repo_path.display());

    let default_config = PeelboxConfig::default();
    let config = PeelboxConfig {
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
        eprintln!("\nPlease check your environment variables and command-line arguments.");
        return 1;
    }

    let wrap_with_recording =
        |client: Arc<dyn peelbox_llm::LLMClient>| -> Arc<dyn peelbox_llm::LLMClient> {
            if std::env::var("PEELBOX_ENABLE_RECORDING").is_ok() {
                let recordings_dir = std::path::PathBuf::from("tests/recordings");
                let mode = RecordingMode::from_env(RecordingMode::Auto);
                match RecordingLLMClient::new(client.clone(), mode, recordings_dir) {
                    Ok(recording_client) => {
                        debug!(
                            "Recording enabled, using tests/recordings/ directory (mode: {:?})",
                            mode
                        );
                        return Arc::new(recording_client) as Arc<dyn peelbox_llm::LLMClient>;
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

        use peelbox_llm::GenAIClient;
        use std::time::Duration;

        let client = match GenAIClient::new(
            config.provider,
            config.model.clone(),
            Duration::from_secs(config.request_timeout_secs),
        )
        .await
        {
            Ok(c) => wrap_with_recording(Arc::new(c) as Arc<dyn peelbox_llm::LLMClient>),
            Err(e) => {
                error!("Failed to initialize backend: {}", e);
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
                eprintln!("  - Run 'peelbox health' to check backend availability");
                eprintln!("  - Or omit --backend to automatically select an available backend");
                return 1;
            }
        };

        DetectionService::new(client)
    } else {
        info!("Using lazy LLM client initialization - backend will be selected on first use");
        let interactive = atty::is(atty::Stream::Stdout);

        // Create lazy client that defers initialization until first chat() call
        let lazy_client = peelbox_llm::LazyLLMClient::new(config.clone(), interactive);
        let client = Arc::new(lazy_client) as Arc<dyn peelbox_llm::LLMClient>;
        let client = wrap_with_recording(client);

        DetectionService::new(client)
    };

    info!(
        "Using backend: {} ({})",
        service.backend_name(),
        service
            .backend_model_info()
            .unwrap_or_else(|| "default".to_string())
    );

    info!("Analyzing repository: {}", repo_path.display());

    let results: Vec<UniversalBuild> = match service.detect(repo_path.clone()).await {
        Ok(r) => r,
        Err(e) => {
            error!("Detection failed: {}", e);
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
                return 1;
            }
        }
    } else {
        println!("{}", output);
    }

    0
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

    let config = PeelboxConfig::default();
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
                "Provider {:?} is not supported by peelbox",
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

async fn handle_build(args: &BuildArgs, quiet: bool, verbose: bool) -> i32 {
    info!("Starting build");

    // Load spec file
    let spec_content = match fs::read_to_string(&args.spec) {
        Ok(content) => content,
        Err(e) => {
            error!("Failed to read spec file {}: {}", args.spec.display(), e);
            return 1;
        }
    };

    // Parse spec (handle both single object and array formats)
    let specs = match serde_json::from_str::<Vec<UniversalBuild>>(&spec_content) {
        Ok(specs) => specs,
        Err(_) => {
            // Try parsing as single object
            match serde_json::from_str::<UniversalBuild>(&spec_content) {
                Ok(s) => vec![s],
                Err(e) => {
                    error!("Failed to parse spec file: {}", e);
                    return 1;
                }
            }
        }
    };

    if specs.is_empty() {
        error!("Spec file contains empty array");
        return 1;
    }

    // Service selection for monorepos
    let spec: UniversalBuild = if specs.len() > 1 {
        // Multiple services detected - require --service flag
        if let Some(ref service_name) = args.service {
            // Collect available services
            let available_services: Vec<String> = specs
                .iter()
                .filter_map(|s| s.metadata.project_name.clone())
                .collect();

            // Find the service by name
            match specs.into_iter().find(|s| {
                s.metadata
                    .project_name
                    .as_ref()
                    .map(|n| n == service_name)
                    .unwrap_or(false)
            }) {
                Some(s) => s,
                None => {
                    error!(
                        "Service '{}' not found. Available services: {}",
                        service_name,
                        available_services.join(", ")
                    );
                    eprintln!(
                        "Error: Service '{}' not found in spec.\n\nAvailable services:\n  {}",
                        service_name,
                        available_services.join("\n  ")
                    );
                    return 1;
                }
            }
        } else {
            // Multiple services but no --service flag
            let service_list: Vec<String> = specs
                .iter()
                .filter_map(|s| s.metadata.project_name.clone())
                .collect();
            error!("Multiple services detected but no --service specified");
            eprintln!(
                "Error: Multiple services detected in spec.\n\nPlease specify which service to build using --service flag:\n  {}",
                service_list.join("\n  ")
            );
            return 1;
        }
    } else {
        // Single service
        specs.into_iter().next().unwrap()
    };

    debug!(
        "Selected spec for project: {:?}",
        spec.metadata.project_name
    );

    // Connect to BuildKit daemon
    info!("Connecting to BuildKit daemon...");
    let connection = match BuildKitConnection::connect(args.buildkit.as_deref()).await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to connect to BuildKit: {}", e);
            return 1;
        }
    };

    info!("Connected to BuildKit successfully");

    // Get build context path (use --context arg or current directory)
    let context_path = args
        .context
        .clone()
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    // Canonicalize context path to ensure deterministic session ID across different ways of specifying the same path
    let context_path = context_path
        .canonicalize()
        .unwrap_or_else(|_| context_path.clone());

    // Determine output destination
    let output_dest = if let Some(output_spec) = &args.output {
        if output_spec == "type=docker"
            || output_spec == "docker"
            || output_spec.starts_with("type=docker,")
        {
            OutputDestination::DockerLoad
        } else {
            let (path_buf, format) = if output_spec == "type=oci" || output_spec == "oci" {
                let sanitized_tag = args.tag.replace([':', '/'], "-");
                (
                    context_path.join(format!("{}.tar", sanitized_tag)),
                    "oci".to_string(),
                )
            } else if let Some(after_type) = output_spec.strip_prefix("type=oci,") {
                let path = if let Some(dest) = after_type.strip_prefix("dest=") {
                    PathBuf::from(dest)
                } else {
                    PathBuf::from(after_type)
                };
                (path, "oci".to_string())
            } else if let Some(dest) = output_spec.strip_prefix("oci,dest=") {
                (PathBuf::from(dest), "oci".to_string())
            } else if let Some(dest) = output_spec.strip_prefix("dest=") {
                (PathBuf::from(dest), "docker".to_string())
            } else {
                (PathBuf::from(output_spec), "docker".to_string())
            };

            OutputDestination::File {
                path: path_buf,
                format,
            }
        }
    } else {
        // Default to Docker daemon load
        OutputDestination::DockerLoad
    };

    info!("Output destination: {}", output_dest);

    // Configure attestations based on CLI flags
    let sbom_enabled = args.sbom && !args.no_sbom;
    let provenance_mode = if args.no_provenance {
        None
    } else if let Some(ref mode_str) = args.provenance {
        match mode_str.to_lowercase().as_str() {
            "min" => Some(ProvenanceMode::Min),
            "max" => Some(ProvenanceMode::Max),
            _ => {
                error!(
                    "Invalid provenance mode '{}'. Valid values: min, max",
                    mode_str
                );
                return 1;
            }
        }
    } else {
        Some(ProvenanceMode::Max) // Default to max
    };

    let attestation_config = AttestationConfig {
        sbom: sbom_enabled,
        provenance: provenance_mode,
        scan_context: args.scan_context,
    };

    if sbom_enabled {
        info!("SBOM attestation enabled (SPDX format)");
    }
    if let Some(mode) = provenance_mode {
        info!("SLSA provenance attestation enabled (mode: {:?})", mode);
    }
    if args.scan_context {
        debug!("Build context scanning enabled for SBOM");
    }

    let session_id = uuid::Uuid::new_v4().to_string();

    // Check for automatic caching via PEELBOX_CACHE_DIR env var
    let cache_base = std::env::var("PEELBOX_CACHE_DIR").ok();
    let using_auto_cache =
        cache_base.is_some() && args.cache_from.is_empty() && args.cache_to.is_empty();

    // Generate app-specific cache key if using auto-cache
    let (auto_cache_dir, auto_cache_key) = if using_auto_cache {
        let base_dir = cache_base.as_ref().unwrap();
        let cache_key = generate_cache_key(&args.spec, &context_path);
        let cache_path = PathBuf::from(base_dir);

        // Create base cache directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&cache_path) {
            warn!(
                "Failed to create cache directory {}: {}",
                cache_path.display(),
                e
            );
            (None, None)
        } else {
            info!(
                "Auto-caching enabled: {} (key: {})",
                cache_path.display(),
                cache_key
            );
            (Some(cache_path), Some(cache_key))
        }
    } else {
        (None, None)
    };

    // Parse cache options (explicit flags take precedence over env var)
    let cache_imports = if !args.cache_from.is_empty() {
        let imports = parse_cache_imports(&args.cache_from, None);
        if imports.is_empty() {
            warn!("No valid cache imports after parsing");
        }
        imports
    } else if let Some(ref cache_dir) = auto_cache_dir {
        // Auto-configure cache import from env var (shared blobs, per-app index)
        parse_cache_imports(
            &[format!("type=local,src={}", cache_dir.display())],
            auto_cache_key.as_deref(),
        )
    } else {
        Vec::new()
    };

    let cache_exports = if !args.cache_to.is_empty() {
        let exports = parse_cache_exports(&args.cache_to);
        if exports.is_empty() {
            warn!("No valid cache exports after parsing");
        }
        exports
    } else if let Some(ref cache_dir) = auto_cache_dir {
        // Auto-configure cache export from env var (shared blobs, per-app index)
        parse_cache_exports(&[format!("type=local,dest={}", cache_dir.display())])
    } else {
        Vec::new()
    };

    let mut session = BuildSession::new(connection, context_path, output_dest)
        .with_attestations(attestation_config)
        .with_session_id(session_id);

    // Set cache key for index file naming (used with local cache)
    if let Some(cache_key) = auto_cache_key {
        session = session.with_cache_key(cache_key);
    }

    // Configure external cache if provided
    if !cache_imports.is_empty() {
        info!("Configuring {} cache import(s)", cache_imports.len());
        session = session.with_cache_imports(cache_imports);
    }
    if !cache_exports.is_empty() {
        info!("Configuring {} cache export(s)", cache_exports.len());
        session = session.with_cache_exports(cache_exports);
    }

    // Initialize session
    if let Err(e) = session.initialize().await {
        error!("Failed to initialize build session: {}", e);
        return 1;
    }

    // Create progress tracker with user-specified verbosity
    let progress_tracker = ProgressTracker::new(quiet, verbose);

    // Execute build
    let result = match session
        .build(&spec, &args.tag, Some(&progress_tracker))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Build failed: {}", e);
            progress_tracker.build_failed(&e.to_string());
            return 1;
        }
    };

    if quiet {
        println!("{}", result.image_id);
    }

    // Progress tracker already printed build completion summary
    debug!("Build completed successfully");
    debug!("Image ID: {}", result.image_id);
    debug!("Image size: {} bytes", result.size_bytes);

    info!("Build successful!");
    info!("  Image: {}", args.tag);
    info!("  ID: {}", result.image_id);
    info!(
        "  Size: {:.2} MB",
        result.size_bytes as f64 / 1024.0 / 1024.0
    );

    0
}

/// Builder for cache options - parses and validates cache configuration
struct CacheOptionBuilder {
    cache_type: String,
    attrs: HashMap<String, String>,
}

impl CacheOptionBuilder {
    fn parse(cache_str: &str) -> anyhow::Result<Self> {
        // Shorthand for registry: "user/app:cache"
        if !cache_str.contains(',') && cache_str.contains('/') {
            return Ok(Self {
                cache_type: "registry".into(),
                attrs: HashMap::from([("ref".into(), cache_str.into())]),
            });
        }

        let mut attrs: HashMap<String, String> = cache_str
            .split(',')
            .filter_map(|pair| {
                pair.split_once('=')
                    .map(|(k, v)| (k.trim().into(), v.trim().into()))
            })
            .collect();

        if attrs.is_empty() {
            anyhow::bail!("Invalid cache option format: {}", cache_str);
        }

        let cache_type = attrs.remove("type").unwrap_or_else(|| "registry".into());
        Ok(Self { cache_type, attrs })
    }

    fn validate(&self, is_export: bool) -> anyhow::Result<()> {
        match self.cache_type.as_str() {
            "registry" => {
                if !self.attrs.contains_key("ref") {
                    anyhow::bail!("Registry cache requires 'ref' attribute");
                }
            }
            "local" => {
                let key = if is_export { "dest" } else { "src" };
                if !self.attrs.contains_key(key) {
                    anyhow::bail!("Local cache requires '{}' attribute", key);
                }
            }
            "gha" | "s3" | "azblob" | "inline" => {}
            unknown => anyhow::bail!("Unknown cache type: {}", unknown),
        }
        Ok(())
    }

    fn into_import(mut self, cache_key: Option<&str>) -> anyhow::Result<CacheImport> {
        self.validate(false)?;

        // Auto-resolve digest for local caches
        if self.cache_type == "local" && !self.attrs.contains_key("digest") {
            if let Some(src) = self.attrs.get("src") {
                match resolve_cache_digest(src, cache_key) {
                    Ok(digest) => {
                        let index_file = peelbox_buildkit::OciIndex::filename(cache_key);
                        info!("Auto-resolved digest from {}: {}", index_file, digest);
                        self.attrs.insert("digest".into(), digest);
                    }
                    Err(e) => {
                        warn!("Failed to auto-resolve digest for {}: {}", src, e);
                    }
                }
            }
        }

        info!("Cache import: type={}, attrs={:?}", self.cache_type, self.attrs);
        Ok(CacheImport {
            r#type: self.cache_type,
            attrs: self.attrs,
        })
    }

    fn into_export(self) -> anyhow::Result<CacheExport> {
        self.validate(true)?;
        info!("Cache export: type={}, attrs={:?}", self.cache_type, self.attrs);
        Ok(CacheExport {
            r#type: self.cache_type,
            attrs: self.attrs,
        })
    }
}

/// Resolve cache digest from index file in the cache directory
fn resolve_cache_digest(cache_dir: &str, cache_key: Option<&str>) -> anyhow::Result<String> {
    use peelbox_buildkit::OciIndex;
    use std::path::PathBuf;

    let cache_path = PathBuf::from(cache_dir);
    let index_file = OciIndex::filename(cache_key);
    let index_path = cache_path.join(&index_file);

    if !index_path.exists() {
        return Err(anyhow::anyhow!("No {} found (first build?)", index_file));
    }

    let index = OciIndex::read_with_key(&cache_path, cache_key)?;

    index
        .get_digest(None)
        .ok_or_else(|| anyhow::anyhow!("No 'latest' tag found in {}", index_file))
}

fn extract_project_name(spec_path: &Path) -> Option<String> {
    let content = fs::read_to_string(spec_path).ok()?;
    let specs: Vec<serde_json::Value> = serde_json::from_str(&content).ok()?;

    specs.first()?
        .get("metadata")?
        .get("project_name")?
        .as_str()
        .map(|s| s.trim().to_lowercase())
}

fn generate_cache_key(spec_path: &Path, context_path: &Path) -> String {
    use sha2::{Digest, Sha256};

    let ctx = context_path.canonicalize().unwrap_or_else(|_| context_path.to_owned());
    let mut hasher = Sha256::new();
    hasher.update(ctx.to_string_lossy().as_bytes());

    if let Some(name) = extract_project_name(spec_path) {
        hasher.update(b":");
        hasher.update(name.as_bytes());
        debug!("Cache key: context + app_name={}", name);
    } else {
        warn!("No app name in spec, using spec path");
        hasher.update(b":");
        hasher.update(spec_path.to_string_lossy().as_bytes());
    }

    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn parse_cache_imports(cache_from: &[String], cache_key: Option<&str>) -> Vec<CacheImport> {
    cache_from
        .iter()
        .filter_map(|s| {
            CacheOptionBuilder::parse(s)
                .and_then(|b| b.into_import(cache_key))
                .map_err(|e| warn!("Failed to parse cache import '{}': {}", s, e))
                .ok()
        })
        .collect()
}

fn parse_cache_exports(cache_to: &[String]) -> Vec<CacheExport> {
    cache_to
        .iter()
        .filter_map(|s| {
            CacheOptionBuilder::parse(s)
                .and_then(|b| b.into_export())
                .map_err(|e| warn!("Failed to parse cache export '{}': {}", s, e))
                .ok()
        })
        .collect()
}
