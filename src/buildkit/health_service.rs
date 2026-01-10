use tonic::{Request, Response, Status};
use tracing::debug;

/// Health service implementation for BuildKit session health monitoring
///
/// BuildKit monitors session health by calling Health.Check every 5 seconds.
/// If two consecutive checks fail, BuildKit closes the session connection.
/// This service must respond to health checks to keep the session alive.
pub struct HealthService;

impl HealthService {
    pub fn new() -> Self {
        Self
    }
}

#[tonic::async_trait]
impl tonic_health::pb::health_server::Health for HealthService {
    type WatchStream = tokio_stream::wrappers::ReceiverStream<
        Result<tonic_health::pb::HealthCheckResponse, Status>,
    >;

    async fn check(
        &self,
        request: Request<tonic_health::pb::HealthCheckRequest>,
    ) -> Result<Response<tonic_health::pb::HealthCheckResponse>, Status> {
        let req = request.into_inner();
        debug!("Health check request for service: {:?}", req.service);

        // Always return SERVING status - session is alive
        let response = tonic_health::pb::HealthCheckResponse {
            status: tonic_health::pb::health_check_response::ServingStatus::Serving as i32,
        };

        Ok(Response::new(response))
    }

    async fn watch(
        &self,
        _request: Request<tonic_health::pb::HealthCheckRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        // Watch is not required for BuildKit sessions
        Err(Status::unimplemented("Health watch not implemented"))
    }
}
