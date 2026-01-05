use anyhow::Result;
use std::path::Path;
use tracing::{debug, info};

const DOCKER_SOCKET_PATH: &str = "/var/run/docker.sock";

/// Check if Docker daemon is available with BuildKit support
pub async fn check_docker_buildkit() -> Result<bool> {
    if !Path::new(DOCKER_SOCKET_PATH).exists() {
        debug!("Docker socket not found at {}", DOCKER_SOCKET_PATH);
        return Ok(false);
    }

    // TODO: Implement Docker API check
    // This would involve:
    // 1. Connect to Docker socket
    // 2. Call /info endpoint
    // 3. Check API version >= 1.41 (Docker 23.0+)
    // 4. Verify BuildKit is enabled

    info!("Docker socket found, assuming BuildKit support (placeholder)");
    Ok(true)
}

/// Get Docker daemon BuildKit endpoint
pub fn get_docker_buildkit_endpoint() -> String {
    // Docker daemon BuildKit typically uses the same socket
    format!("unix://{}", DOCKER_SOCKET_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_docker_buildkit() {
        // This will succeed or fail based on whether Docker is running
        let result = check_docker_buildkit().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_docker_endpoint() {
        let endpoint = get_docker_buildkit_endpoint();
        assert!(endpoint.starts_with("unix://"));
        assert!(endpoint.contains("docker.sock"));
    }
}
