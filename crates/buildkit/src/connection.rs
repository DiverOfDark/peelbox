use anyhow::{Context, Result};
use std::path::Path;
#[cfg(not(windows))]
use tonic::transport::Uri;
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, info};

use crate::retry::retry_with_backoff;

const DEFAULT_UNIX_SOCKET: &str = "/run/buildkit/buildkitd.sock";
const DEFAULT_DOCKER_SOCKET: &str = "/var/run/docker.sock";
const MIN_BUILDKIT_VERSION: &str = "0.11.0";

/// Consistent keep-alive timeout applied to all BuildKit connections.
///
/// Set to 600 seconds (10 minutes). This is long enough to survive typical
/// build pauses (e.g. waiting for a large image pull), but short enough to
/// detect genuinely dead connections before the OS TCP timeout kicks in.
const KEEP_ALIVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// Configure a tonic `Endpoint` with the standard HTTP/2 settings used by
/// all BuildKit connection variants.
///
/// This ensures consistent window sizes, keep-alive intervals, and timeouts
/// across Unix, TCP, Docker-container, and Docker-native connections.
fn configure_endpoint(ep: Endpoint) -> Endpoint {
    ep.http2_adaptive_window(true)
        .initial_connection_window_size(Some(2 * 1024 * 1024))
        .initial_stream_window_size(Some(2 * 1024 * 1024))
        .http2_keep_alive_interval(std::time::Duration::from_secs(30))
        .keep_alive_timeout(KEEP_ALIVE_TIMEOUT)
        .keep_alive_while_idle(true)
}

#[derive(Debug, Clone)]
pub enum BuildKitAddr {
    Unix(String),
    Tcp(String),
    DockerContainer(String),
    DockerNative(String),
}

impl BuildKitAddr {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(addr: &str) -> Result<Self> {
        if let Some(path) = addr.strip_prefix("unix://") {
            Ok(BuildKitAddr::Unix(path.to_string()))
        } else if let Some(path) = addr.strip_prefix("docker-container://") {
            Ok(BuildKitAddr::DockerContainer(path.to_string()))
        } else if let Some(path) = addr.strip_prefix("docker://") {
            let socket_path = if path.is_empty() {
                DEFAULT_DOCKER_SOCKET
            } else {
                path
            };
            Ok(BuildKitAddr::DockerNative(socket_path.to_string()))
        } else if addr.starts_with("tcp://") {
            Ok(BuildKitAddr::Tcp(addr.to_string()))
        } else {
            anyhow::bail!(
                "Invalid BuildKit address format. Expected unix://, tcp://, docker:// or docker-container://"
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
    pub async fn connect(addr: Option<&str>) -> Result<Self> {
        if let Some(addr_str) = addr {
            let addr = BuildKitAddr::from_str(addr_str)?;
            info!("Connecting to BuildKit at explicit address: {:?}", addr);
            Self::connect_to_addr(addr).await
        } else {
            Self::auto_detect().await
        }
    }

    async fn auto_detect() -> Result<Self> {
        debug!("Auto-detecting BuildKit daemon...");

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

        debug!("Trying Docker daemon...");
        match super::docker::detect_docker_buildkit_endpoint().await {
            Ok(Some(endpoint)) => {
                debug!("Found Docker BuildKit endpoint: {}", endpoint);
                match BuildKitAddr::from_str(&endpoint) {
                    Ok(addr) => {
                        let connect_result = retry_with_backoff(
                            5,
                            std::time::Duration::from_secs(1),
                            "Docker BuildKit connect",
                            || {
                                let addr = addr.clone();
                                async move { Self::connect_to_addr(addr).await }
                            },
                        )
                        .await;

                        match connect_result {
                            Ok(conn) => {
                                info!("Connected to Docker daemon BuildKit");
                                return Ok(conn);
                            }
                            Err(e) => {
                                debug!("All Docker BuildKit connect attempts failed: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to parse Docker endpoint: {}", e);
                    }
                }
            }
            Ok(None) => {
                debug!("No BuildKit container found in Docker");
            }
            Err(e) => {
                debug!("Failed to detect Docker BuildKit: {}", e);
            }
        }

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

    async fn connect_to_addr(addr: BuildKitAddr) -> Result<Self> {
        let channel = match &addr {
            BuildKitAddr::Unix(path) => {
                debug!("Connecting to Unix socket: {}", path);

                #[cfg(unix)]
                {
                    use hyper_util::rt::TokioIo;
                    use tower::service_fn;

                    let path = path.clone();
                    configure_endpoint(
                        Endpoint::try_from("http://[::]:50051")
                            .context("Failed to create endpoint")?,
                    )
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
                configure_endpoint(Endpoint::try_from(uri_str.clone()).context("Invalid TCP URI")?)
                    .connect_timeout(std::time::Duration::from_secs(10))
                    .timeout(std::time::Duration::from_secs(3600))
                    .connect()
                    .await
                    .context("Failed to connect to TCP endpoint")?
            }
            BuildKitAddr::DockerContainer(container_id) => {
                debug!("Connecting to Docker container: {}", container_id);

                #[cfg(unix)]
                {
                    use tower::service_fn;

                    let container_id = container_id.clone();
                    configure_endpoint(
                        Endpoint::try_from("http://[::]:50051")
                            .context("Failed to create endpoint")?,
                    )
                    .timeout(std::time::Duration::from_secs(3600))
                    .connect_with_connector(service_fn(move |_: Uri| {
                        let container_id = container_id.clone();
                        async move { connect_docker_container(&container_id).await }
                    }))
                    .await
                    .context("Failed to connect to BuildKit via docker-container")?
                }

                #[cfg(not(unix))]
                {
                    anyhow::bail!("Docker container connections require Unix platform");
                }
            }
            BuildKitAddr::DockerNative(path) => {
                debug!("Connecting to Docker native (POST /grpc): {}", path);

                #[cfg(unix)]
                {
                    use hyper_util::rt::TokioIo;
                    use tower::service_fn;

                    let path = path.clone();
                    configure_endpoint(
                        Endpoint::try_from("http://[::]:50051")
                            .context("Failed to create endpoint")?,
                    )
                    .connect_with_connector(service_fn(move |_: Uri| {
                        let path = path.clone();
                        async move { connect_docker_native(&path).await.map(TokioIo::new) }
                    }))
                    .await
                    .context("Failed to connect to Docker native socket")?
                }

                #[cfg(not(unix))]
                {
                    anyhow::bail!("Docker native connections require Unix platform");
                }
            }
        };

        let mut conn = Self { channel, addr };

        conn.health_check().await?;

        conn.version_check().await?;

        Ok(conn)
    }

    async fn health_check(&mut self) -> Result<()> {
        use crate::proto::ControlClient;

        let channel = self.channel.clone();
        retry_with_backoff(
            10,
            std::time::Duration::from_secs(1),
            "BuildKit health check",
            || {
                let mut client = ControlClient::new(channel.clone());
                async move {
                    let request =
                        crate::proto::moby::buildkit::v1::ListWorkersRequest { filter: vec![] };
                    let resp = client
                        .list_workers(request)
                        .await
                        .map_err(|e| anyhow::anyhow!("{}", e.message()))?;
                    let workers = resp.into_inner().record.len();
                    info!("BuildKit health check passed ({} workers)", workers);
                    Ok::<_, anyhow::Error>(())
                }
            },
        )
        .await
        .map_err(|_| anyhow::anyhow!("BuildKit not ready after 10 health check attempts"))
    }

    async fn version_check(&mut self) -> Result<()> {
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
async fn connect_docker_native(path: &str) -> std::io::Result<tokio::net::UnixStream> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    let mut stream = UnixStream::connect(path).await?;

    let request = "POST /grpc HTTP/1.1\r\n\
                   Host: docker\r\n\
                   Connection: Upgrade\r\n\
                   Upgrade: h2c\r\n\
                   \r\n";

    stream.write_all(request.as_bytes()).await?;

    let mut reader = tokio::io::BufReader::new(stream);
    let mut line = String::new();

    reader.read_line(&mut line).await?;
    if !line.starts_with("HTTP/1.1 101") {
        return Err(std::io::Error::other(format!(
            "Docker daemon failed to switch protocols: {}",
            line.trim()
        )));
    }

    loop {
        line.clear();
        reader.read_line(&mut line).await?;
        if line == "\r\n" {
            break;
        }
    }

    if reader.buffer().is_empty() {
        // After the HTTP upgrade handshake, the Unix socket is a plain
        // bidirectional stream.  Pass it directly to tonic — no duplex
        // proxy needed.  A duplex proxy deadlocks because both copy
        // directions share one buffer and can fill it simultaneously.
        Ok(reader.into_inner())
    } else {
        Err(std::io::Error::other(
            "Buffered data remaining after handshake",
        ))
    }
}

#[cfg(unix)]
async fn connect_docker_container(
    container_id: &str,
) -> std::io::Result<hyper_util::rt::TokioIo<tokio::io::DuplexStream>> {
    use bollard::Docker;

    let docker = Docker::connect_with_local_defaults()
        .map_err(|e| std::io::Error::other(format!("Failed to connect to Docker: {}", e)))?;

    let container_info = docker
        .inspect_container(container_id, None)
        .await
        .map_err(|e| std::io::Error::other(format!("Failed to inspect container: {}", e)))?;

    let port_opt = container_info
        .network_settings
        .and_then(|ns| ns.ports)
        .and_then(|ports| ports.get("1234/tcp").cloned())
        .and_then(|bindings| bindings)
        .and_then(|mut b| b.pop())
        .and_then(|binding| binding.host_port)
        .and_then(|port| port.parse::<u16>().ok());

    if let Some(port) = port_opt {
        return Err(std::io::Error::other(format!(
            "BuildKit container has TCP port exposed on localhost:{}. \
                 Use --buildkit tcp://127.0.0.1:{} instead of docker-container://",
            port, port
        )));
    }

    Err(std::io::Error::other(
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
