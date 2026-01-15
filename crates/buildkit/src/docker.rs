use anyhow::Result;
use bollard::container::ListContainersOptions;
use bollard::Docker;
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

const DOCKER_SOCKET_PATH: &str = "/var/run/docker.sock";

pub async fn detect_docker_buildkit_endpoint() -> Result<Option<String>> {
    if !Path::new(DOCKER_SOCKET_PATH).exists() {
        debug!("Docker socket not found at {}", DOCKER_SOCKET_PATH);
        return Ok(None);
    }

    let docker = match Docker::connect_with_local_defaults() {
        Ok(d) => d,
        Err(e) => {
            debug!("Failed to connect to Docker: {}", e);
            return Ok(None);
        }
    };

    match docker.version().await {
        Ok(v) => {
            let api_version = v.api_version.unwrap_or_else(|| "0.0".to_string());
            if let Ok(version_float) = api_version.parse::<f32>() {
                if version_float >= 1.41 {
                    debug!(
                        "Docker API version {} supports native BuildKit via POST /grpc",
                        api_version
                    );
                    return Ok(Some(format!("docker://{}", DOCKER_SOCKET_PATH)));
                } else {
                    debug!("Docker API version {} is too old for native BuildKit. Requires 1.41+ (Docker 23.0+)", api_version);
                }
            }
        }
        Err(e) => {
            debug!("Failed to get Docker version: {}", e);
            return Ok(None);
        }
    }

    let mut filters = HashMap::new();
    filters.insert("status", vec!["running"]);

    let options = Some(ListContainersOptions {
        all: true,
        filters,
        ..Default::default()
    });

    match docker.list_containers(options).await {
        Ok(containers) => {
            for container in containers {
                let is_buildkit = container
                    .image
                    .as_ref()
                    .map(|img| img.contains("moby/buildkit") || img.contains("buildkit"))
                    .unwrap_or(false);

                if is_buildkit {
                    if let Some(names) = container.names {
                        if let Some(name) = names.first() {
                            let clean_name = name.trim_start_matches('/');
                            debug!("Found BuildKit container: {}", clean_name);
                            return Ok(Some(format!("docker-container://{}", clean_name)));
                        }
                    }
                }
            }
        }
        Err(e) => {
            debug!("Failed to list containers: {}", e);
        }
    }

    debug!("No running BuildKit containers found");
    Ok(None)
}

pub async fn check_docker_buildkit() -> Result<bool> {
    Ok(detect_docker_buildkit_endpoint().await?.is_some())
}

pub fn get_docker_buildkit_endpoint() -> String {
    format!("unix://{}", DOCKER_SOCKET_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_docker_buildkit() {
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
