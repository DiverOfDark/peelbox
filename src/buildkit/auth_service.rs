use anyhow::Result;
use tonic::{Request, Response, Status};
use tracing::{debug, warn};

use super::proto::moby::filesync::v1::{
    CredentialsRequest, CredentialsResponse, FetchTokenRequest, FetchTokenResponse,
    GetTokenAuthorityRequest, GetTokenAuthorityResponse, VerifyTokenAuthorityRequest,
    VerifyTokenAuthorityResponse,
};
use super::proto::AuthServer;

/// Auth service implementation for BuildKit session
/// Handles registry authentication during image pulls/pushes
pub struct AuthService {}

impl AuthService {
    pub fn new() -> Self {
        Self {}
    }
}

#[tonic::async_trait]
impl AuthServer for AuthService {
    async fn credentials(
        &self,
        request: Request<CredentialsRequest>,
    ) -> Result<Response<CredentialsResponse>, Status> {
        let req = request.into_inner();
        debug!("Auth.Credentials called for host: {}", req.host);

        // Return empty credentials (anonymous access)
        // BuildKit will use anonymous pull for public images
        Ok(Response::new(CredentialsResponse {
            username: String::new(),
            secret: String::new(),
        }))
    }

    async fn fetch_token(
        &self,
        request: Request<FetchTokenRequest>,
    ) -> Result<Response<FetchTokenResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "Auth.FetchToken called for host: {}, realm: {}, service: {}",
            req.host, req.realm, req.service
        );

        // Return Unimplemented to let BuildKit use anonymous/credentials auth
        // Similar to GetTokenAuthority, we don't support token-based auth
        Err(Status::unimplemented(
            "Token-based auth not supported - use anonymous or credentials",
        ))
    }

    async fn get_token_authority(
        &self,
        request: Request<GetTokenAuthorityRequest>,
    ) -> Result<Response<GetTokenAuthorityResponse>, Status> {
        let req = request.into_inner();
        debug!(
            "Auth.GetTokenAuthority called for host: {}, salt len: {}",
            req.host,
            req.salt.len()
        );

        // Return Unimplemented to indicate token authority is not supported
        // This allows BuildKit to fall back to anonymous or credentials-based auth
        Err(Status::unimplemented("Token authority not supported"))
    }

    async fn verify_token_authority(
        &self,
        request: Request<VerifyTokenAuthorityRequest>,
    ) -> Result<Response<VerifyTokenAuthorityResponse>, Status> {
        let req = request.into_inner();
        warn!(
            "Auth.VerifyTokenAuthority called for host: {} - not implemented",
            req.host
        );

        // Return empty signature (verification not supported)
        Ok(Response::new(VerifyTokenAuthorityResponse {
            signed: vec![],
        }))
    }
}
