use anyhow::Result;
use tonic::{Request, Response, Status};
use tracing::debug;

use super::proto::moby::exporter::v1::exporter_server::Exporter as ExporterTrait;
use super::proto::moby::exporter::v1::{
    ExporterRequest, FindExportersRequest, FindExportersResponse,
};

/// Exporter gRPC service implementation
///
/// Handles exporter discovery for BuildKit daemon.
/// When enable_session_exporter is true, BuildKit calls FindExporters
/// to discover available exporters from the session.
pub struct ExporterService {
    image_tag: String,
    exporter_type: String,
}

impl ExporterService {
    pub fn new(image_tag: String, exporter_type: String) -> Self {
        debug!("Creating ExporterService with tag={}, type={}", image_tag, exporter_type);
        Self {
            image_tag,
            exporter_type,
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

        // Return our tar-based exporter configuration
        let exporter = ExporterRequest {
            r#type: self.exporter_type.clone(),
            attrs: [
                ("name".to_string(), self.image_tag.clone()),
                ("tar".to_string(), "true".to_string()),
            ]
            .into_iter()
            .collect(),
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
