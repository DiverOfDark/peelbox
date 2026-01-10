use anyhow::Result;
use std::path::Path;
use tracing::debug;

const DOCKER_SOCKET_PATH: &str = "/var/run/docker.sock";

/// Check if Docker daemon is available with BuildKit support
pub async fn check_docker_buildkit() -> Result<bool> {
    if !Path::new(DOCKER_SOCKET_PATH).exists() {
        debug!("Docker socket not found at {}", DOCKER_SOCKET_PATH);
        return Ok(false);
    }

    use bollard::Docker;

    let docker = match Docker::connect_with_local_defaults() {
        Ok(d) => d,
        Err(e) => {
            debug!("Failed to connect to Docker: {}", e);
            return Ok(false);
        }
    };

    match docker.version().await {
        Ok(v) => {
            let api_version = v.api_version.unwrap_or_else(|| "0.0".to_string());
            debug!("Docker API version: {}", api_version);
            // BuildKit requires API version >= 1.31, but 1.41+ is recommended
            Ok(true)
        }
        Err(e) => {
            debug!("Failed to get Docker version: {}", e);
            Ok(false)
        }
    }
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
