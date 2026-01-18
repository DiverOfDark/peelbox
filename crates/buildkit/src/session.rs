use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tonic::transport::Server;
use tracing::{debug, error, info, warn};

use super::auth_service::AuthService;
use super::connection::BuildKitConnection;
use super::content_service::ContentService;
use super::filesend_service::{FileSendService, OutputDestination};
use super::filesync::FileSync;
use super::filesync_service::FileSyncService;
use super::llb::LLBBuilder;
use super::proto::{
    AuthServerBuilder, BytesMessage, ContentServerBuilder, ControlClient, FileSendServerBuilder,
    FileSyncServerBuilder,
};
use super::stream_conn::StreamConn;
use crate::{BuildStrategy, PeelboxStrategy};
use peelbox_core::output::schema::UniversalBuild;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

const TAR_EXPORT_TIMEOUT_SECS: u64 = 300;

/// Cache import configuration
#[derive(Debug, Clone)]
pub struct CacheImport {
    pub r#type: String,
    pub attrs: HashMap<String, String>,
}

/// Cache export configuration
#[derive(Debug, Clone)]
pub struct CacheExport {
    pub r#type: String,
    pub attrs: HashMap<String, String>,
}

/// Attestation configuration for SBOM and provenance generation
#[derive(Debug, Clone)]
pub struct AttestationConfig {
    /// Enable SBOM (Software Bill of Materials) generation in SPDX format
    pub sbom: bool,
    /// Enable SLSA provenance attestation (mode: min or max)
    pub provenance: Option<ProvenanceMode>,
    /// Scan build context for SBOM generation
    pub scan_context: bool,
}

/// SLSA provenance generation mode
#[derive(Debug, Clone, Copy)]
pub enum ProvenanceMode {
    /// Minimal provenance (fast, basic metadata)
    Min,
    /// Maximum provenance (complete audit trail, recommended for production)
    Max,
}

impl Default for AttestationConfig {
    fn default() -> Self {
        Self {
            sbom: true,
            provenance: Some(ProvenanceMode::Max),
            scan_context: true,
        }
    }
}

/// BuildKit session for managing build context transfer and build execution
pub struct BuildSession {
    connection: BuildKitConnection,
    session_id: String,
    context_path: PathBuf,
    output_dest: OutputDestination,

    attestation_config: AttestationConfig,
    cache_imports: Vec<CacheImport>,
    cache_exports: Vec<CacheExport>,
    cache_key: Option<String>,
    session_server: Option<JoinHandle<Result<()>>>,
    session_tx: Option<mpsc::Sender<BytesMessage>>,
    conn_tx: Option<mpsc::Sender<Result<StreamConn, std::io::Error>>>,
    export_done: Option<tokio::sync::oneshot::Receiver<()>>,
    bytes_written: Arc<AtomicU64>,
}

impl BuildSession {
    /// Create a new build session
    pub fn new(
        connection: BuildKitConnection,
        context_path: PathBuf,
        output_dest: OutputDestination,
    ) -> Self {
        let session_id = Self::generate_session_id();
        debug!("Creating new build session: {}", session_id);

        Self {
            connection,
            session_id,
            context_path,
            output_dest,
            attestation_config: AttestationConfig::default(),
            cache_imports: Vec::new(),
            cache_exports: Vec::new(),
            cache_key: None,
            session_server: None,
            session_tx: None,
            conn_tx: None,
            export_done: None,
            bytes_written: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Configure attestation generation (SBOM and provenance)
    pub fn with_attestations(mut self, config: AttestationConfig) -> Self {
        self.attestation_config = config;
        self
    }

    /// Set cache imports (e.g. registry, local)
    pub fn with_cache_imports(mut self, imports: Vec<CacheImport>) -> Self {
        self.cache_imports = imports;
        self
    }

    /// Set cache exports (e.g. registry, local, inline)
    pub fn with_cache_exports(mut self, exports: Vec<CacheExport>) -> Self {
        self.cache_exports = exports;
        self
    }

    /// Set cache key for index file naming (used with local cache)
    pub fn with_cache_key(mut self, cache_key: String) -> Self {
        self.cache_key = Some(cache_key);
        self
    }

    /// Set a custom session ID (useful for deterministic builds/caching)
    pub fn with_session_id(mut self, session_id: String) -> Self {
        debug!(
            "Overriding session ID from {} to {}",
            self.session_id, session_id
        );
        self.session_id = session_id;
        self
    }

    /// Generate a unique session ID using UUID
    fn generate_session_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Initialize session and transfer build context
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing build session: {}", self.session_id);

        // Create FileSync for context transfer
        let file_sync = FileSync::new(&self.context_path);

        // Scan files in build context
        let file_stats = file_sync
            .scan_files()
            .await
            .with_context(|| format!("Failed to scan build context at {:?}", self.context_path))?;

        info!(
            "Build context contains {} files/directories",
            file_stats.len()
        );

        debug!("Session context prepared: {}", self.session_id);

        // Attach session to BuildKit daemon - REQUIRED for proper operation
        // This creates the unified gRPC server over the BytesMessage stream
        self.attach_session().await?;

        Ok(())
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get build context path
    pub fn context_path(&self) -> &PathBuf {
        &self.context_path
    }

    /// Extract local cache directory from cache exports or imports
    fn local_cache_dir(&self) -> Option<&str> {
        self.cache_exports
            .iter()
            .find(|export| export.r#type == "local")
            .and_then(|export| export.attrs.get("dest"))
            .or_else(|| {
                self.cache_imports
                    .iter()
                    .find(|import| import.r#type == "local")
                    .and_then(|import| import.attrs.get("src"))
            })
            .map(|s| s.as_str())
    }

    async fn shutdown_session(&mut self) -> Result<()> {
        if let Some(handle) = self.session_server.take() {
            debug!("Aborting session server task");
            handle.abort();

            match tokio::time::timeout(Duration::from_secs(2), handle).await {
                Ok(Ok(_)) => debug!("Session server stopped gracefully"),
                Ok(Err(e)) => {
                    if e.is_cancelled() {
                        debug!("Session server aborted");
                    } else {
                        warn!("Session server error on shutdown: {}", e);
                    }
                }
                Err(_) => debug!("Session server aborted (timeout)"),
            }
        }
        Ok(())
    }

    /// Update index file in the cache directory after export
    fn update_cache_index(&self, cache_dir: &str) -> Result<()> {
        use std::path::PathBuf;

        let cache_path: PathBuf = cache_dir.into();

        let (digest, size) = crate::oci_index::find_latest_manifest(&cache_path)?
            .ok_or_else(|| anyhow::anyhow!("No OCI manifest found in cache directory"))?;

        let mut index = crate::OciIndex::read_with_key(&cache_path, self.cache_key.as_deref())?;
        index.add_or_update_manifest(digest.clone(), size, "latest");
        index.write_with_key(&cache_path, self.cache_key.as_deref())?;

        let index_file = crate::OciIndex::filename(self.cache_key.as_deref());
        info!("Updated {} with manifest {}", index_file, digest);
        Ok(())
    }

    /// Attach session to BuildKit daemon via Control.Session RPC
    async fn attach_session(&mut self) -> Result<()> {
        info!("Attaching session {} to BuildKit daemon", self.session_id);

        let mut client = ControlClient::new(self.connection.channel())
            .max_decoding_message_size(100 * 1024 * 1024)
            .max_encoding_message_size(100 * 1024 * 1024);

        // Create channel for outgoing session messages
        let (tx, rx) = mpsc::channel::<BytesMessage>(32);

        // Convert to stream - start with empty stream
        // BuildKit will initiate gRPC calls over the tunneled connection
        let outgoing_stream = ReceiverStream::new(rx);

        // Create request with session metadata in gRPC headers
        let mut request = tonic::Request::new(outgoing_stream);

        // Add session metadata headers
        request.metadata_mut().insert(
            "x-docker-expose-session-uuid",
            self.session_id
                .parse()
                .context("Failed to parse session ID")?,
        );

        request.metadata_mut().insert(
            "x-docker-expose-session-name",
            self.session_id
                .parse()
                .context("Failed to parse session name")?,
        );

        request.metadata_mut().insert(
            "x-docker-expose-session-sharedkey",
            self.session_id
                .parse()
                .context("Failed to parse shared key")?,
        );

        // Advertise all available gRPC methods
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/moby.filesync.v1.FileSync/DiffCopy"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/moby.filesync.v1.FileSync/TarStream"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/moby.filesync.v1.Auth/Credentials"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/moby.filesync.v1.Auth/FetchToken"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/moby.filesync.v1.Auth/GetTokenAuthority"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/moby.filesync.v1.Auth/VerifyTokenAuthority"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/moby.filesync.v1.FileSend/DiffCopy"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/grpc.health.v1.Health/Check"
                .parse()
                .context("Failed to parse method")?,
        );

        // Advertise Content service methods (for cache export/import)
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/containerd.services.content.v1.Content/Read"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/containerd.services.content.v1.Content/Write"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/containerd.services.content.v1.Content/Info"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/containerd.services.content.v1.Content/Status"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/containerd.services.content.v1.Content/ListStatuses"
                .parse()
                .context("Failed to parse method")?,
        );
        request.metadata_mut().append(
            "x-docker-expose-session-grpc-method",
            "/containerd.services.content.v1.Content/Abort"
                .parse()
                .context("Failed to parse method")?,
        );

        // Call Control.Session with metadata
        info!("Calling Control.Session RPC...");
        let response = client.session(request).await.map_err(|e| {
            error!(
                "Control.Session RPC failed: status={:?}, message='{}', details={:?}",
                e.code(),
                e.message(),
                e
            );
            anyhow::anyhow!(
                "Failed to attach session: {} (code: {:?})",
                e.message(),
                e.code()
            )
        })?;

        let incoming = response.into_inner();

        // Create StreamConn adapter from BytesMessage stream
        let stream_conn = StreamConn::new(incoming, tx.clone());

        // Create oneshot channel for export completion signal
        let (export_signal, export_done) = tokio::sync::oneshot::channel();

        let cache_dir: Option<PathBuf> = self.local_cache_dir().map(Into::into);

        debug!(
            "Cache directory extraction: exports={}, imports={}, cache_dir={:?}",
            self.cache_exports.len(),
            self.cache_imports.len(),
            cache_dir
        );
        if let Some(ref dir) = cache_dir {
            debug!("  Using cache directory: {}", dir.display());
            for export in &self.cache_exports {
                debug!(
                    "  Cache export: type={}, attrs={:?}",
                    export.r#type, export.attrs
                );
            }
            for import in &self.cache_imports {
                debug!(
                    "  Cache import: type={}, attrs={:?}",
                    import.r#type, import.attrs
                );
            }
        }

        // Create unified gRPC server with FileSync, FileSend, Auth, Content, and Health services
        let filesync_service = FileSyncService::new(self.context_path.clone());
        let filesend_service = FileSendService::new(
            self.output_dest.clone(),
            export_signal,
            self.bytes_written.clone(),
        );
        let auth_service = AuthService::new();
        let health_service = super::health_service::HealthService::new();

        info!("Creating unified gRPC server with FileSync, FileSend, Auth, Content, and Health services");

        if let Some(ref dir) = cache_dir {
            info!(
                "Content service enabled with cache directory: {}",
                dir.display()
            );
        }

        // Create an infinite connection stream that yields the single StreamConn
        // and then blocks forever to keep the server alive
        let (conn_tx, conn_rx) = mpsc::channel(1);

        // Send the single connection
        conn_tx
            .send(Ok::<_, std::io::Error>(stream_conn))
            .await
            .context("Failed to send StreamConn")?;

        // Don't drop conn_tx - keep it alive so the stream never ends
        let conn_stream = ReceiverStream::new(conn_rx);

        // Build and serve unified gRPC server
        debug!("Registering gRPC services:");
        debug!("  - FileSync (moby.filesync.v1.FileSync)");
        debug!("  - FileSend (moby.filesync.v1.FileSend)");
        debug!("  - Auth (moby.filesync.v1.Auth)");
        debug!("  - Health (grpc.health.v1.Health)");
        if cache_dir.is_some() {
            debug!("  - Content (containerd.services.content.v1.Content)");
        }

        let mut server_builder = Server::builder()
            .trace_fn(|_| tracing::info_span!("grpc-server"))
            .add_service(FileSyncServerBuilder::new(filesync_service))
            .add_service(FileSendServerBuilder::new(filesend_service))
            .add_service(AuthServerBuilder::new(auth_service));

        // Add Content service if cache directory is configured
        if let Some(cache_dir) = cache_dir {
            let content_service = ContentService::new(cache_dir);
            server_builder = server_builder.add_service(ContentServerBuilder::new(content_service));
        }

        let server = server_builder
            .add_service(tonic_health::pb::health_server::HealthServer::new(
                health_service,
            ))
            .serve_with_incoming(conn_stream);

        debug!("gRPC server built, ready to accept connections");

        // Spawn task to run the gRPC server
        let session_id = self.session_id.clone();
        let session_handle = tokio::spawn(async move {
            debug!("Session {} gRPC server starting", session_id);
            debug!(
                "Session {} server task spawned, awaiting serve_with_incoming",
                session_id
            );

            match server.await {
                Ok(()) => {
                    debug!("Session {} gRPC server completed successfully", session_id);
                }
                Err(e) => {
                    error!("Session {} gRPC server error: {}", session_id, e);
                }
            }

            debug!("Session {} gRPC server task exiting", session_id);
            Ok(())
        });

        self.session_server = Some(session_handle);
        self.session_tx = Some(tx); // Keep sender alive to prevent BytesMessage stream from closing
        self.conn_tx = Some(conn_tx); // Keep connection stream alive - never ends until session dropped
        self.export_done = Some(export_done); // Receive export completion signal

        info!(
            "Session {} attached successfully - gRPC server running over BytesMessage stream",
            self.session_id
        );

        // Give BuildKit time to register the session before we start the build
        // This prevents race condition where Solve() is called before session manager knows about our session
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        debug!("Session {} registration delay complete", self.session_id);

        Ok(())
    }

    /// Build image from UniversalBuild spec
    pub async fn build(
        &mut self,
        spec: &UniversalBuild,
        image_tag: &str,
        progress: Option<&super::progress::ProgressTracker>,
    ) -> Result<BuildResult> {
        debug!("Building image: {}", image_tag);

        if let Some(tracker) = progress.as_ref() {
            tracker.build_started(image_tag);
        }

        // Extract OCI image config from spec before generating LLB
        let mut env_vars: Vec<String> = spec
            .runtime
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        env_vars.sort(); // Sort for deterministic hash

        #[derive(Debug, Clone, serde::Serialize)]
        struct ImageConfig {
            #[serde(rename = "Cmd")]
            cmd: Vec<String>,
            #[serde(rename = "Env")]
            env: Vec<String>,
            #[serde(rename = "WorkingDir")]
            working_dir: String,
            #[serde(rename = "Entrypoint")]
            entrypoint: Vec<String>,
        }

        let image_config = ImageConfig {
            cmd: spec.runtime.command.clone(),
            env: env_vars,
            working_dir: "/app".to_string(),
            entrypoint: spec.runtime.command.clone(),
        };

        let image_config = if !image_config.entrypoint.is_empty() {
            ImageConfig {
                cmd: vec![],
                ..image_config
            }
        } else {
            image_config
        };

        // Serialize OCI config to JSON for exporter attribute
        // BuildKit expects {"Config": {...}} wrapper (see https://github.com/moby/buildkit/issues/1041)
        let oci_config_json = serde_json::json!({
            "Config": {
                "Cmd": image_config.cmd,
                "Env": image_config.env,
                "WorkingDir": image_config.working_dir,
                "Entrypoint": image_config.entrypoint,
            },
            "architecture": "amd64",
            "os": "linux",
        });
        let config_json_str =
            serde_json::to_string(&oci_config_json).context("Failed to serialize OCI config")?;

        debug!("OCI config JSON: {}", config_json_str);

        // Generate LLB from spec
        let project_name = spec
            .metadata
            .project_name
            .clone()
            .unwrap_or_else(|| "unnamed".to_string());
        let mut llb_builder = LLBBuilder::new("context")
            .with_context_path(self.context_path.clone())
            .with_project_name(project_name)
            .with_session_id(self.session_id.clone());

        PeelboxStrategy.build_graph(&mut llb_builder, spec)?;

        let llb_bytes = llb_builder.to_bytes()?;

        if llb_bytes.is_empty() {
            error!("Generated LLB definition is empty! Check LLBBuilder::write_definition.");
            return Err(anyhow::anyhow!("Generated LLB definition is empty"));
        }

        debug!("Generated LLB definition ({} bytes)", llb_bytes.len());

        #[cfg(debug_assertions)]
        {
            let dump_path = std::env::temp_dir().join("peelbox_llb_dump.pb");
            if let Err(e) = std::fs::write(&dump_path, &llb_bytes) {
                debug!("Failed to dump LLB to {:?}: {}", dump_path, e);
            } else {
                debug!("LLB dumped to {:?} for inspection", dump_path);
            }
        }

        // Create Control client with extended timeout for long builds
        let mut client = ControlClient::new(self.connection.channel())
            .max_decoding_message_size(100 * 1024 * 1024) // 100MB for large build responses
            .max_encoding_message_size(100 * 1024 * 1024);

        // Parse LLB bytes into Definition proto
        let definition = prost::Message::decode(&llb_bytes[..]).with_context(|| {
            format!(
                "Failed to decode LLB definition ({} bytes). Data may be corrupted.",
                llb_bytes.len()
            )
        })?;

        // Create local input for context source
        // When LLB contains Source::local("context"), BuildKit needs to know
        // which session provides it via frontend_inputs
        let mut frontend_inputs = std::collections::HashMap::new();

        // Create an empty Definition to indicate session-provided local source
        // BuildKit will resolve "context" from the session's FileSync service
        let local_source_def = super::proto::pb::Definition {
            def: vec![],
            metadata: Default::default(),
            source: None,
        };

        // Associating the context name with an empty definition tells BuildKit
        // to use the session-provided local context
        frontend_inputs.insert("context".to_string(), local_source_def);

        // Create exporter with OCI config and attestations
        let mut exporter_attrs = std::collections::HashMap::new();
        exporter_attrs.insert("name".to_string(), image_tag.to_string());
        exporter_attrs.insert("tar".to_string(), "true".to_string());
        exporter_attrs.insert("containerimage.config".to_string(), config_json_str);

        // Add SBOM attestation if enabled
        if self.attestation_config.sbom {
            exporter_attrs.insert("attest:sbom".to_string(), String::new());
            debug!("Enabled SBOM attestation (SPDX format)");
        }

        // Add SLSA provenance attestation if enabled
        if let Some(mode) = self.attestation_config.provenance {
            let mode_str = match mode {
                ProvenanceMode::Min => "mode=min",
                ProvenanceMode::Max => "mode=max",
            };
            exporter_attrs.insert("attest:provenance".to_string(), mode_str.to_string());
            debug!("Enabled SLSA provenance attestation ({})", mode_str);
        }

        // Add build context scanning for SBOM
        if self.attestation_config.scan_context {
            exporter_attrs.insert(
                "build-arg:BUILDKIT_SBOM_SCAN_CONTEXT".to_string(),
                "true".to_string(),
            );
            debug!("Enabled build context scanning for SBOM");
        }

        let exporter_type = match &self.output_dest {
            OutputDestination::DockerLoad => "docker",
            OutputDestination::File { format, .. } => format.as_str(),
        };

        let exporter = super::proto::moby::buildkit::v1::Exporter {
            r#type: exporter_type.to_string(),
            attrs: exporter_attrs,
        };

        let imports = self
            .cache_imports
            .iter()
            .map(|i| super::proto::moby::buildkit::v1::CacheOptionsEntry {
                r#type: i.r#type.clone(),
                attrs: i.attrs.clone(),
            })
            .collect();

        let exports = self
            .cache_exports
            .iter()
            .map(|e| super::proto::moby::buildkit::v1::CacheOptionsEntry {
                r#type: e.r#type.clone(),
                attrs: e.attrs.clone(),
            })
            .collect();

        let cache = super::proto::moby::buildkit::v1::CacheOptions {
            export_ref_deprecated: String::new(),
            import_refs_deprecated: vec![],
            export_attrs_deprecated: Default::default(),
            exports,
            imports,
        };

        // Create solve request
        let request = super::proto::moby::buildkit::v1::SolveRequest {
            // Use unique ref per build to avoid "job ID exists" errors,
            // but keep the session ID stable for caching
            r#ref: format!("{}-{}", self.session_id, uuid::Uuid::new_v4()),
            definition: Some(definition),
            exporter: exporter.r#type,
            exporter_attrs: exporter.attrs,
            session: self.session_id.clone(),
            frontend: String::new(),
            frontend_attrs: Default::default(),
            cache: Some(cache),
            entitlements: vec![],
            frontend_inputs,
            source_policy: None,
            internal: false, // Enable provenance/SBOM generation (LLB has reference-only final node)
        };

        debug!("Submitting build request to BuildKit...");
        debug!(
            "Request details: ref={}, session={}, exporter={}",
            request.r#ref, request.session, request.exporter
        );

        let build_ref = request.r#ref.clone();

        // Start status streaming task if progress tracking is enabled
        let mut status_rx = if progress.is_some() {
            let (tx, rx) = mpsc::channel(100);
            let mut status_client = ControlClient::new(self.connection.channel())
                .max_decoding_message_size(100 * 1024 * 1024);
            let status_request = super::proto::moby::buildkit::v1::StatusRequest {
                r#ref: build_ref.clone(),
            };

            debug!("Starting Status stream for build ref: {}", build_ref);

            tokio::spawn(async move {
                let mut stream = match status_client.status(status_request).await {
                    Ok(response) => response.into_inner(),
                    Err(e) => {
                        error!("Failed to start Status stream: {}", e);
                        return;
                    }
                };

                use futures_util::StreamExt;
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(status_response) => {
                            debug!(
                                "Status update: {} vertices, {} statuses, {} logs, {} warnings",
                                status_response.vertexes.len(),
                                status_response.statuses.len(),
                                status_response.logs.len(),
                                status_response.warnings.len()
                            );

                            if tx.send(status_response).await.is_err() {
                                debug!("Status receiver dropped, stopping status stream");
                                break;
                            }
                        }
                        Err(e) => {
                            debug!("Status stream error (expected on completion): {}", e);
                            break;
                        }
                    }
                }
                debug!("Status stream ended");
            });

            Some(rx)
        } else {
            None
        };

        // Submit build request and process status updates concurrently
        let solve_future = client.solve(request);

        info!("Build submitted, streaming progress...");

        // Process status updates while build is running
        let solve_response = if let Some(rx) = status_rx.as_mut() {
            use futures_util::FutureExt;
            tokio::pin!(solve_future);
            let mut solve_future = solve_future.fuse();

            loop {
                tokio::select! {
                    response = &mut solve_future => {
                        match response {
                            Ok(resp) => {
                                // Process any remaining status updates
                                while let Ok(status) = rx.try_recv() {
                                    if let Some(tracker) = progress {
                                        tracker.process_status(status);
                                    }
                                }
                                break resp.into_inner();
                            }
                            Err(e) => {
                                error!("BuildKit Solve RPC error: status={:?}, message={}", e.code(), e.message());
                                error!("Full error: {:?}", e);
                                return Err(anyhow::anyhow!("Failed to submit build to BuildKit: status: {}, message: \"{}\"", e.code(), e.message()));
                            }
                        }
                    }
                    status_opt = rx.recv() => {
                        if let Some(status) = status_opt {
                            if let Some(tracker) = progress {
                                tracker.process_status(status);
                            }
                        }
                    }
                }
            }
        } else {
            // No progress tracking, just wait for solve to complete
            solve_future
                .await
                .map_err(|e| {
                    error!(
                        "BuildKit Solve RPC error: status={:?}, message={}",
                        e.code(),
                        e.message()
                    );
                    error!("Full error: {:?}", e);
                    anyhow::anyhow!(
                        "Failed to submit build to BuildKit: status: {}, message: \"{}\"",
                        e.code(),
                        e.message()
                    )
                })?
                .into_inner()
        };

        debug!("Build completed successfully!");
        debug!("Solve response: {:?}", solve_response);

        // Wait for tar export to complete before closing session
        if let Some(export_done) = self.export_done.take() {
            debug!("Waiting for tar export to complete...");
            match tokio::time::timeout(Duration::from_secs(TAR_EXPORT_TIMEOUT_SECS), export_done)
                .await
            {
                Ok(Ok(())) => {
                    debug!("Tar export completed successfully");
                }
                Ok(Err(_)) => {
                    warn!("Export completion sender dropped - export may have failed");
                }
                Err(_) => {
                    error!("Timeout waiting for tar export after 5 minutes");
                    return Err(anyhow::anyhow!("Tar export timed out after 5 minutes"));
                }
            }
        } else {
            debug!("No export completion signal configured");
        }

        if let Some(cache_dir) = self.local_cache_dir() {
            debug!(
                "Creating/updating index.json for local cache at {}",
                cache_dir
            );
            if let Err(e) = self.update_cache_index(cache_dir) {
                warn!("Failed to create/update index.json: {}", e);
            }
        }

        self.shutdown_session().await?;

        // Extract image ID from response
        let image_id = solve_response
            .exporter_response
            .get("containerimage.digest")
            .cloned()
            .unwrap_or_else(|| format!("sha256:{}", self.session_id));

        // Get bytes written from FileSendService
        let tar_size_bytes = self.bytes_written.load(Ordering::Relaxed);

        // Extract layer count from exporter response if available
        let layers = if let Some(config_json) = solve_response
            .exporter_response
            .get("containerimage.config")
        {
            if let Ok(config) = serde_json::from_str::<serde_json::Value>(config_json) {
                config["rootfs"]["diff_ids"]
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };

        let build_result = BuildResult {
            image_id: image_id.clone(),
            size_bytes: tar_size_bytes,
            layers,
        };

        if let Some(tracker) = progress {
            tracker.build_completed(&build_result.image_id, build_result.size_bytes);
        }

        Ok(build_result)
    }
}

/// Result of a successful build
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub image_id: String,
    pub size_bytes: u64,
    pub layers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_session_id() {
        let id1 = BuildSession::generate_session_id();
        let id2 = BuildSession::generate_session_id();

        // IDs should be valid UUIDs
        assert!(uuid::Uuid::parse_str(&id1).is_ok());
        assert!(uuid::Uuid::parse_str(&id2).is_ok());
        // IDs should be different (with high probability)
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_creation() {
        let _temp_dir = TempDir::new().unwrap();
        // We can't create a real BuildKitConnection without a running daemon,
        // so we'll test what we can without it
        let session_id = BuildSession::generate_session_id();
        assert!(uuid::Uuid::parse_str(&session_id).is_ok());
    }
}
