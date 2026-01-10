use anyhow::{Context, Result};
use std::path::Path;
use tonic::transport::{Channel, Endpoint, Uri};
use tracing::{debug, info};

const DEFAULT_UNIX_SOCKET: &str = "/run/buildkit/buildkitd.sock";
const DEFAULT_DOCKER_SOCKET: &str = "/var/run/docker.sock";
const MIN_BUILDKIT_VERSION: &str = "0.11.0";

#[derive(Debug, Clone)]
pub enum BuildKitAddr {
    Unix(String),
    Tcp(String),
    DockerContainer(String),
}

impl BuildKitAddr {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(addr: &str) -> Result<Self> {
        if let Some(path) = addr.strip_prefix("unix://") {
            Ok(BuildKitAddr::Unix(path.to_string()))
        } else if let Some(path) = addr.strip_prefix("docker-container://") {
            Ok(BuildKitAddr::DockerContainer(path.to_string()))
        } else if addr.starts_with("tcp://") {
            Ok(BuildKitAddr::Tcp(addr.to_string()))
        } else {
            anyhow::bail!(
                "Invalid BuildKit address format. Expected unix://, tcp://, or docker-container://"
            )
        }
    }

    pub fn default_unix() -> Self {
        BuildKitAddr::Unix(DEFAULT_UNIX_SOCKET.to_string())
    }

    pub fn docker_socket() -> Self {
        BuildKitAddr::Unix(DEFAULT_DOCKER_SOCKET.to_string())
    }
}

pub struct BuildKitConnection {
    channel: Channel,
    addr: BuildKitAddr,
}

impl BuildKitConnection {
    /// Connect to BuildKit daemon with auto-detection
    pub async fn connect(addr: Option<&str>) -> Result<Self> {
        if let Some(addr_str) = addr {
            // Explicit address provided
            let addr = BuildKitAddr::from_str(addr_str)?;
            info!("Connecting to BuildKit at explicit address: {:?}", addr);
            Self::connect_to_addr(addr).await
        } else {
            // Auto-detect
            Self::auto_detect().await
        }
    }

    /// Auto-detect BuildKit daemon
    async fn auto_detect() -> Result<Self> {
        debug!("Auto-detecting BuildKit daemon...");

        // Try 1: Unix socket (standalone BuildKit)
        let unix_addr = BuildKitAddr::default_unix();
        if Path::new(DEFAULT_UNIX_SOCKET).exists() {
            debug!(
                "Found standalone BuildKit socket at {}",
                DEFAULT_UNIX_SOCKET
            );
            match Self::connect_to_addr(unix_addr.clone()).await {
                Ok(conn) => {
                    info!("Connected to standalone BuildKit daemon");
                    return Ok(conn);
                }
                Err(e) => {
                    debug!("Failed to connect to standalone BuildKit: {}", e);
                }
            }
        } else {
            debug!(
                "Standalone BuildKit socket not found at {}",
                DEFAULT_UNIX_SOCKET
            );
        }

        // Try 2: Docker daemon
        debug!("Trying Docker daemon...");
        if let Ok(has_docker) = super::docker::check_docker_buildkit().await {
            if has_docker {
                let docker_endpoint = super::docker::get_docker_buildkit_endpoint();
                match BuildKitAddr::from_str(&docker_endpoint) {
                    Ok(addr) => match Self::connect_to_addr(addr).await {
                        Ok(conn) => {
                            info!("Connected to Docker daemon BuildKit");
                            return Ok(conn);
                        }
                        Err(e) => {
                            debug!("Failed to connect to Docker daemon: {}", e);
                        }
                    },
                    Err(e) => {
                        debug!("Failed to parse Docker endpoint: {}", e);
                    }
                }
            }
        }

        // No BuildKit found
        anyhow::bail!(
            "Failed to connect to BuildKit daemon\n\n\
            Tried:\n\
            ✗ Unix socket: {} ({})\n\
            ✗ Docker daemon: {} (not available or no BuildKit support)\n\n\
            Install BuildKit:\n\
              macOS:  brew install buildkit\n\
              Linux:  sudo apt install buildkit\n\
              Docker: Upgrade to Docker Desktop 4.17+ or Docker Engine 23.0+\n\n\
            Or start standalone BuildKit:\n\
              docker run -d --privileged -p 1234:1234 moby/buildkit:latest --addr tcp://0.0.0.0:1234\n\
              peelbox build --buildkit tcp://127.0.0.1:1234 ...",
            DEFAULT_UNIX_SOCKET,
            if Path::new(DEFAULT_UNIX_SOCKET).exists() { "connection failed" } else { "not found" },
            DEFAULT_DOCKER_SOCKET
        );
    }

    /// Connect to specific BuildKit address
    async fn connect_to_addr(addr: BuildKitAddr) -> Result<Self> {
        let channel = match &addr {
            BuildKitAddr::Unix(path) => {
                debug!("Connecting to Unix socket: {}", path);

                #[cfg(unix)]
                {
                    use hyper_util::rt::TokioIo;
                    use tower::service_fn;

                    let path = path.clone();
                    Endpoint::try_from("http://[::]:50051")
                        .context("Failed to create endpoint")?
                        // Configure HTTP/2 settings for BuildKit compatibility
                        .http2_adaptive_window(true)
                        .initial_connection_window_size(Some(2 * 1024 * 1024)) // 2MB
                        .initial_stream_window_size(Some(2 * 1024 * 1024)) // 2MB
                        .http2_keep_alive_interval(std::time::Duration::from_secs(30))
                        .connect_with_connector(service_fn(move |_: Uri| {
                            let path = path.clone();
                            async move {
                                tokio::net::UnixStream::connect(path)
                                    .await
                                    .map(TokioIo::new)
                            }
                        }))
                        .await
                        .context("Failed to connect to Unix socket")?
                }

                #[cfg(not(unix))]
                {
                    anyhow::bail!("Unix sockets not supported on this platform");
                }
            }
            BuildKitAddr::Tcp(uri_str) => {
                debug!("Connecting to TCP: {}", uri_str);
                Endpoint::try_from(uri_str.clone())
                    .context("Invalid TCP URI")?
                    .connect()
                    .await
                    .context("Failed to connect to TCP endpoint")?
            }
            BuildKitAddr::DockerContainer(container_id) => {
                debug!("Connecting to Docker container: {}", container_id);

                // BuildKit in Docker container exposes Unix socket at /run/buildkit/buildkitd.sock
                // We connect via docker exec to access the socket inside the container

                #[cfg(unix)]
                {
                    use tower::service_fn;

                    let container_id = container_id.clone();
                    Endpoint::try_from("http://[::]:50051")
                        .context("Failed to create endpoint")?
                        .connect_with_connector(service_fn(move |_: Uri| {
                            let container_id = container_id.clone();
                            async move {
                                // Use docker exec to access BuildKit socket inside container
                                // This creates a proxy connection: local -> docker exec -> buildkit socket
                                connect_docker_container(&container_id).await
                            }
                        }))
                        .await
                        .context("Failed to connect to BuildKit via docker-container")?
                }

                #[cfg(not(unix))]
                {
                    anyhow::bail!("Docker container connections require Unix platform");
                }
            }
        };

        let mut conn = Self { channel, addr };

        // Health check
        conn.health_check().await?;

        // Version check
        conn.version_check().await?;

        Ok(conn)
    }

    /// Health check using BuildKit Control service
    async fn health_check(&mut self) -> Result<()> {
        // This will be implemented when we add the Control service client
        debug!("Health check: OK (placeholder)");
        Ok(())
    }

    /// Check BuildKit version
    async fn version_check(&mut self) -> Result<()> {
        // This will be implemented when we add the Control service client
        debug!(
            "Version check: OK (placeholder - will require v{}+)",
            MIN_BUILDKIT_VERSION
        );
        Ok(())
    }

    pub fn channel(&self) -> Channel {
        self.channel.clone()
    }

    pub fn addr(&self) -> &BuildKitAddr {
        &self.addr
    }
}

#[cfg(unix)]
async fn connect_docker_container(
    container_id: &str,
) -> std::io::Result<hyper_util::rt::TokioIo<tokio::io::DuplexStream>> {
    use bollard::Docker;

    // Docker exec doesn't provide true bidirectional streams needed for HTTP/2
    // Instead, we'll use the Docker API to find or create a port binding for BuildKit

    let docker = Docker::connect_with_local_defaults().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to connect to Docker: {}", e),
        )
    })?;

    // Inspect container to see if BuildKit port is already exposed
    let container_info = docker
        .inspect_container(container_id, None)
        .await
        .map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to inspect container: {}", e),
            )
        })?;

    // Check if port 1234 (BuildKit default) is mapped
    let port_opt = container_info
        .network_settings
        .and_then(|ns| ns.ports)
        .and_then(|ports| ports.get("1234/tcp").cloned())
        .and_then(|bindings| bindings)
        .and_then(|mut b| b.pop())
        .and_then(|binding| binding.host_port)
        .and_then(|port| port.parse::<u16>().ok());

    if let Some(port) = port_opt {
        // Port is already exposed, return error telling user to use tcp:// instead
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "BuildKit container has TCP port exposed on localhost:{}. \
                 Use --buildkit tcp://127.0.0.1:{} instead of docker-container://",
                port, port
            ),
        ));
    }

    // Port not exposed - docker exec is the only option, but it doesn't work for gRPC
    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!(
            "BuildKit container '{}' does not expose TCP port.\n\n\
             Docker exec cannot provide bidirectional streams required for HTTP/2 gRPC.\n\n\
             Please restart BuildKit container with TCP port exposed:\n\
             docker run -d --rm --privileged -p 127.0.0.1:1234:1234 moby/buildkit:latest --addr tcp://0.0.0.0:1234\n\n\
             Then use: --buildkit tcp://127.0.0.1:1234",
            container_id
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buildkit_addr_parsing() {
        let unix = BuildKitAddr::from_str("unix:///run/buildkit/buildkitd.sock").unwrap();
        assert!(matches!(unix, BuildKitAddr::Unix(_)));

        let tcp = BuildKitAddr::from_str("tcp://127.0.0.1:1234").unwrap();
        assert!(matches!(tcp, BuildKitAddr::Tcp(_)));

        let docker = BuildKitAddr::from_str("docker-container://buildkitd").unwrap();
        assert!(matches!(docker, BuildKitAddr::DockerContainer(_)));

        assert!(BuildKitAddr::from_str("invalid://addr").is_err());
    }

    #[test]
    fn test_default_addresses() {
        let unix = BuildKitAddr::default_unix();
        assert!(matches!(unix, BuildKitAddr::Unix(ref path) if path == DEFAULT_UNIX_SOCKET));

        let docker = BuildKitAddr::docker_socket();
        assert!(matches!(docker, BuildKitAddr::Unix(ref path) if path == DEFAULT_DOCKER_SOCKET));
    }
}
