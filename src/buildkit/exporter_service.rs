use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tonic::{Request, Response, Status};
use tracing::debug;

use super::proto::moby::exporter::v1::exporter_server::Exporter as ExporterTrait;
use super::proto::moby::exporter::v1::{
    ExporterRequest, FindExportersRequest, FindExportersResponse,
};

/// OCI image configuration for runtime
#[derive(Clone)]
pub struct ImageConfig {
    pub cmd: Vec<String>,
    pub env: Vec<String>,
    pub working_dir: String,
    pub entrypoint: Vec<String>,
}

/// Exporter gRPC service implementation
///
/// Handles exporter discovery for BuildKit daemon.
/// When enable_session_exporter is true, BuildKit calls FindExporters
/// to discover available exporters from the session.
pub struct ExporterService {
    image_tag: String,
    exporter_type: String,
    config: Arc<Mutex<Option<ImageConfig>>>,
}

impl ExporterService {
    pub fn new(image_tag: String, exporter_type: String, config: Arc<Mutex<Option<ImageConfig>>>) -> Self {
        debug!("Creating ExporterService with tag={}, type={}", image_tag, exporter_type);
        Self {
            image_tag,
            exporter_type,
            config,
        }
    }
}

#[tonic::async_trait]
impl ExporterTrait for ExporterService {
    async fn find_exporters(
        &self,
        request: Request<FindExportersRequest>,
    ) -> Result<Response<FindExportersResponse>, Status> {
        debug!("!!! Exporter::FindExporters HANDLER CALLED !!!");
        debug!("FindExporters request metadata: {:?}", request.metadata());

        let req = request.into_inner();

        debug!(
            "Exporter::FindExporters processing request with {} refs",
            req.refs.len()
        );

        // Build exporter attributes
        let mut attrs: HashMap<String, String> = [
            ("name".to_string(), self.image_tag.clone()),
            ("tar".to_string(), "true".to_string()),
        ]
        .into_iter()
        .collect();

        // Add OCI image config if provided
        if let Ok(guard) = self.config.lock() {
            if let Some(config) = guard.as_ref() {
                // BuildKit expects OCI Image Spec Config JSON with required os/architecture
                let oci_config = serde_json::json!({
                    "Cmd": config.cmd,
                    "Env": config.env,
                    "WorkingDir": config.working_dir,
                    "Entrypoint": config.entrypoint,
                    "architecture": "amd64",
                    "os": "linux",
                });

                let config_json = serde_json::to_string(&oci_config)
                    .map_err(|e| Status::internal(format!("Failed to serialize config: {}", e)))?;

                attrs.insert("containerimage.config".to_string(), config_json);
                debug!("Added OCI config to exporter: cmd={:?}, env={:?}", config.cmd, config.env);
            }
        }

        // Return our tar-based exporter configuration
        let exporter = ExporterRequest {
            r#type: self.exporter_type.clone(),
            attrs,
        };

        let response = FindExportersResponse {
            exporters: vec![exporter],
        };

        debug!(
            "Returning {} exporter for session-based export",
            self.exporter_type
        );

        Ok(Response::new(response))
    }
}
