use peelbox_buildkit::filesend_service::OutputDestination;
use peelbox_buildkit::{
    progress::ProgressTracker, AttestationConfig, BuildKitConnection, BuildSession, CacheExport,
    CacheImport, ProvenanceMode,
};
use peelbox_cli::cli::commands::{BuildArgs, CliArgs, Commands, DetectArgs};
use peelbox_cli::cli::output::{OutputFormat, OutputFormatter};
use peelbox_cli::{NAME, VERSION};
use peelbox_core::output::schema::UniversalBuild;
use peelbox_pipeline::detection::service::DetectionService;

use clap::Parser;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    init_logging_from_args(&args);

    debug!("{} v{} starting", NAME, VERSION);
    debug!("Arguments: {:?}", args);

    let exit_code = match &args.command {
        Commands::Detect(detect_args) => handle_detect(detect_args, args.quiet),
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

fn handle_detect(args: &DetectArgs, quiet: bool) -> i32 {
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

    info!("Analyzing repository: {}", repo_path.display());

    let service = DetectionService::new();
    let results: Vec<UniversalBuild> = match service.detect(repo_path.clone()) {
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

    let spec_path = args
        .spec
        .canonicalize()
        .unwrap_or_else(|_| args.spec.clone());
    let spec_path_str = spec_path.to_string_lossy().to_string();

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
                    dest.into()
                } else {
                    after_type.into()
                };
                (path, "oci".to_string())
            } else if let Some(dest) = output_spec.strip_prefix("oci,dest=") {
                (dest.into(), "oci".to_string())
            } else if let Some(dest) = output_spec.strip_prefix("dest=") {
                (dest.into(), "docker".to_string())
            } else {
                (output_spec.into(), "docker".to_string())
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
    let using_auto_cache = cache_base.is_some() && args.cache.is_empty();

    // Generate app-specific cache key if using auto-cache
    let (auto_cache_dir, auto_cache_key) = if using_auto_cache {
        let base_dir = cache_base.as_ref().unwrap();
        let cache_key = generate_cache_key(&args.spec, &context_path);
        let cache_path: PathBuf = base_dir.into();

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
    let (cache_imports, cache_exports) = if !args.cache.is_empty() || auto_cache_dir.is_some() {
        // Only require project_name when caching is enabled
        let project_name = match spec.metadata.project_name.as_deref() {
            Some(name) => name,
            None => {
                error!("Project name is required when caching is enabled");
                return 1;
            }
        };

        if !args.cache.is_empty() {
            let imports = parse_cache_imports(
                &args.cache,
                auto_cache_key.as_deref(),
                project_name,
                &spec_path_str,
            );
            let exports = parse_cache_exports(&args.cache);
            if imports.is_empty() && exports.is_empty() {
                warn!("No valid cache configurations after parsing");
            }
            (imports, exports)
        } else if let Some(ref cache_dir) = auto_cache_dir {
            // Auto-configure cache from env var (shared blobs, per-app index)
            let cache_import_str = format!("type=local,src={}", cache_dir.display());
            let cache_export_str = format!("type=local,dest={}", cache_dir.display());
            let imports = parse_cache_imports(
                std::slice::from_ref(&cache_import_str),
                auto_cache_key.as_deref(),
                project_name,
                &spec_path_str,
            );
            let exports = parse_cache_exports(&[cache_export_str]);
            (imports, exports)
        } else {
            unreachable!("Cache logic should only execute when cache is configured")
        }
    } else {
        (Vec::new(), Vec::new())
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
        .build(&spec, &spec_path_str, &args.tag, Some(&progress_tracker))
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

/// Cache configuration parser and validator
struct CacheConfig {
    r#type: String,
    attrs: HashMap<String, String>,
}

impl CacheConfig {
    fn parse(cache_str: &str) -> anyhow::Result<Self> {
        // Shorthand for registry: "user/app:cache"
        if !cache_str.contains(',') && cache_str.contains('/') {
            return Ok(Self {
                r#type: "registry".into(),
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
        Ok(Self {
            r#type: cache_type,
            attrs,
        })
    }

    fn validate_import(&self) -> anyhow::Result<()> {
        match self.r#type.as_str() {
            "registry" => Self::ensure_attr(&self.attrs, "ref"),
            "local" => Self::ensure_attr(&self.attrs, "src"),
            "gha" | "s3" | "azblob" | "inline" => Ok(()),
            unknown => anyhow::bail!("Unknown cache type: {}", unknown),
        }
    }

    fn validate_export(&self) -> anyhow::Result<()> {
        match self.r#type.as_str() {
            "registry" => Self::ensure_attr(&self.attrs, "ref"),
            "local" => Self::ensure_attr(&self.attrs, "dest"),
            "gha" | "s3" | "azblob" | "inline" => Ok(()),
            unknown => anyhow::bail!("Unknown cache type: {}", unknown),
        }
    }

    fn ensure_attr(attrs: &HashMap<String, String>, key: &str) -> anyhow::Result<()> {
        attrs
            .contains_key(key)
            .then_some(())
            .ok_or_else(|| anyhow::anyhow!("Missing required attribute: {}", key))
    }

    fn into_import(
        mut self,
        cache_key: Option<&str>,
        application_name: &str,
        universal_build_path: &str,
    ) -> anyhow::Result<CacheImport> {
        // Translate path= to src= for local caches before validation
        if self.r#type == "local"
            && self.attrs.contains_key("path")
            && !self.attrs.contains_key("src")
        {
            if let Some(path) = self.attrs.remove("path") {
                self.attrs.insert("src".into(), path);
            }
        }

        self.validate_import()?;

        // Auto-resolve digest for local caches
        if self.r#type == "local" && !self.attrs.contains_key("digest") {
            let src = self.attrs.get("src").or_else(|| self.attrs.get("path"));
            if let Some(src) = src {
                match resolve_cache_digest(src, application_name, universal_build_path) {
                    Ok(digest) => {
                        let index_file = peelbox_buildkit::OciIndex::filename(cache_key);
                        info!("Auto-resolved digest from {}: {}", index_file, digest);
                        self.attrs.insert("digest".into(), digest);
                    }
                    Err(e) => {
                        warn!("Failed to auto-resolve digest for {}: {}. Skipping cache import (first build or cache miss).", src, e);
                        return Err(anyhow::anyhow!(
                            "Local cache import requires digest, but could not resolve from {}: {}",
                            src,
                            e
                        ));
                    }
                }
            }
        }

        info!("Cache import: type={}, attrs={:?}", self.r#type, self.attrs);
        Ok(CacheImport {
            r#type: self.r#type,
            attrs: self.attrs,
        })
    }

    fn into_export(mut self) -> anyhow::Result<CacheExport> {
        // Translate path= to dest= for local caches before validation
        if self.r#type == "local"
            && self.attrs.contains_key("path")
            && !self.attrs.contains_key("dest")
        {
            if let Some(path) = self.attrs.remove("path") {
                self.attrs.insert("dest".into(), path);
            }
        }

        self.validate_export()?;
        info!("Cache export: type={}, attrs={:?}", self.r#type, self.attrs);
        Ok(CacheExport {
            r#type: self.r#type,
            attrs: self.attrs,
        })
    }
}

/// Resolve cache digest from index file in the cache directory
fn resolve_cache_digest(
    cache_dir: &str,
    application_name: &str,
    universal_build_path: &str,
) -> anyhow::Result<String> {
    use peelbox_buildkit::OciIndex;
    use std::path::Path;

    let index = OciIndex::read_with_lock(Path::new(cache_dir))?;
    index
        .get_digest(None, application_name, universal_build_path)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No 'latest' tag in cache index for application {}",
                application_name
            )
        })
}

fn extract_project_name(spec_path: &Path) -> Option<String> {
    let content = fs::read_to_string(spec_path).ok()?;
    let specs: Vec<serde_json::Value> = serde_json::from_str(&content).ok()?;

    specs
        .first()?
        .get("metadata")?
        .get("project_name")?
        .as_str()
        .map(|s| s.trim().to_lowercase())
}

fn generate_cache_key(spec_path: &Path, context_path: &Path) -> String {
    use sha2::{Digest, Sha256};

    let ctx = context_path
        .canonicalize()
        .unwrap_or_else(|_| context_path.to_owned());
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

fn parse_cache_imports(
    cache_from: &[String],
    cache_key: Option<&str>,
    application_name: &str,
    universal_build_path: &str,
) -> Vec<CacheImport> {
    cache_from
        .iter()
        .filter_map(|s| {
            CacheConfig::parse(s)
                .and_then(|b| b.into_import(cache_key, application_name, universal_build_path))
                .map_err(|e| warn!("Failed to parse cache import '{}': {}", s, e))
                .ok()
        })
        .collect()
}

fn parse_cache_exports(cache_to: &[String]) -> Vec<CacheExport> {
    cache_to
        .iter()
        .filter_map(|s| {
            CacheConfig::parse(s)
                .and_then(|b| b.into_export())
                .map_err(|e| warn!("Failed to parse cache export '{}': {}", s, e))
                .ok()
        })
        .collect()
}
