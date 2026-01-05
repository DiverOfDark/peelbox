use anyhow::{Context, Result};
use bollard::container::{Config, LogsOptions, RemoveContainerOptions, StartContainerOptions};
use bollard::Docker;
use futures_util::stream::StreamExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::core::{Mount, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::OnceCell;
use tokio::time::timeout;

/// Global shared BuildKit container for parallel tests
///
/// Uses a single BuildKit container instance across all parallel test builds to:
/// - Avoid lock conflicts on /var/lib/buildkit/buildkitd.lock
/// - Reduce container startup overhead
/// - Share build cache across all tests
/// - Enable parallel builds (BuildKit handles concurrent build requests)
///
/// Stores (TCP port, container ID)
static BUILDKIT_CONTAINER: OnceCell<Arc<(u16, String, ContainerAsync<GenericImage>)>> =
    OnceCell::const_new();

/// Fixed container name for the shared BuildKit instance
const BUILDKIT_CONTAINER_NAME: &str = "peelbox-test-buildkit";

/// Get or create the shared BuildKit container
///
/// This function is thread-safe and will only create one BuildKit container
/// for all parallel tests across all test binaries. Subsequent calls return
/// the existing TCP port and container ID.
///
/// Uses a fixed container name to enable reuse across test binaries.
/// Returns (TCP port, container ID)
pub async fn get_buildkit_container() -> Result<(u16, String)> {
    let docker = Docker::connect_with_local_defaults().context("Failed to connect to Docker")?;

    // Check if container already exists (may be from another test binary)
    match docker
        .inspect_container(BUILDKIT_CONTAINER_NAME, None)
        .await
    {
        Ok(inspect) => {
            // Container exists, check if it's running
            if inspect.state.and_then(|s| s.running) == Some(true) {
                // Container is running, get its port mapping
                let port = inspect
                    .network_settings
                    .and_then(|ns| ns.ports)
                    .and_then(|ports| ports.get("1234/tcp").cloned())
                    .and_then(|bindings| bindings)
                    .and_then(|mut b| b.pop())
                    .and_then(|binding| binding.host_port)
                    .and_then(|port| port.parse::<u16>().ok())
                    .context("Failed to get BuildKit port from existing container")?;

                let container_id = inspect.id.context("Container ID missing")?;
                return Ok((port, container_id));
            } else {
                // Container exists but not running, remove it
                let _ = docker
                    .remove_container(
                        BUILDKIT_CONTAINER_NAME,
                        Some(bollard::container::RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await;
            }
        }
        Err(_) => {
            // Container doesn't exist, we'll create it below
        }
    }

    let container = BUILDKIT_CONTAINER
        .get_or_init(|| async {
            // Use .buildkit-cache in project root (separate from target/ to avoid Rust cache conflicts)
            let cache_dir = std::env::current_dir()
                .expect("Failed to get current directory")
                .join(".buildkit-cache");
            std::fs::create_dir_all(&cache_dir).expect("Failed to create BuildKit cache directory");

            // Start new BuildKit container with TCP port exposed
            // Map port 1234 to random host port and run BuildKit with TCP listener
            let buildkit_container: ContainerAsync<GenericImage> = GenericImage::new("moby/buildkit", "latest")
                .with_wait_for(WaitFor::message_on_stderr("running server on"))
                .with_privileged(true)
                .with_mount(Mount::bind_mount(
                    cache_dir.to_str().expect("Invalid cache path"),
                    "/var/lib/buildkit",
                ))
                .with_container_name(BUILDKIT_CONTAINER_NAME)
                .with_mapped_port(0, 1234.into())
                .with_cmd(vec!["--addr", "tcp://0.0.0.0:1234"])
                .start()
                .await
                .expect("Failed to start BuildKit container");

            let container_id = buildkit_container.id().to_string();

            // Get the mapped host port for BuildKit TCP
            let port: u16 = buildkit_container.get_host_port_ipv4(1234).await.expect("Failed to get BuildKit host port");

            // Small delay to ensure BuildKit is fully ready
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Store container to keep it alive for the duration of the test run
            Arc::new((port, container_id, buildkit_container))
        })
        .await;

    Ok((container.0, container.1.clone()))
}

/// Container test harness for building and running images from UniversalBuild specs
pub struct ContainerTestHarness {
    docker: Docker,
}

#[allow(dead_code)]
impl ContainerTestHarness {
    /// Create a new harness instance
    pub fn new() -> Result<Self> {
        let docker =
            Docker::connect_with_local_defaults().context("Failed to connect to Docker/Podman")?;
        Ok(Self { docker })
    }

    /// Build a container image from a UniversalBuild JSON spec using peelbox build
    /// Returns the image name
    /// Uses BuildKit gRPC protocol directly via TCP (no buildctl needed)
    pub async fn build_image(
        &self,
        spec_path: &Path,
        _context_path: &Path,
        image_name: &str,
    ) -> Result<String> {
        // Get or create the shared BuildKit container
        let (port, _container_id) = get_buildkit_container().await?;

        // Build peelbox binary if not already built
        let peelbox_binary = std::env::current_dir()
            .context("Failed to get current directory")?
            .join("target/release/peelbox");

        if !peelbox_binary.exists() {
            let build_status = std::process::Command::new("cargo")
                .args([
                    "build",
                    "--release",
                    "--bin",
                    "peelbox",
                    "--no-default-features",
                ])
                .status()
                .context("Failed to build peelbox")?;

            if !build_status.success() {
                anyhow::bail!("Failed to build peelbox binary");
            }
        }

        // Build image using peelbox build command (direct BuildKit gRPC via TCP)
        let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

        let peelbox_output = std::process::Command::new(&peelbox_binary)
            .args([
                "build",
                "--spec",
                spec_path.to_str().unwrap(),
                "--tag",
                image_name,
                "--buildkit",
                &buildkit_addr,
            ])
            .output()
            .context("Failed to run peelbox build")?;

        if !peelbox_output.status.success() {
            eprintln!(
                "peelbox build stdout:\n{}",
                String::from_utf8_lossy(&peelbox_output.stdout)
            );
            eprintln!(
                "peelbox build stderr:\n{}",
                String::from_utf8_lossy(&peelbox_output.stderr)
            );
            anyhow::bail!(
                "peelbox build failed: {}",
                String::from_utf8_lossy(&peelbox_output.stderr)
            );
        }

        // Verify image exists in Docker registry
        self.docker
            .inspect_image(image_name)
            .await
            .context("Failed to inspect image after build")?;

        Ok(image_name.to_string())
    }

    /// Start a container from an image with dynamic port binding
    /// Returns the container ID
    /// The container_port will be bound to a random available host port
    pub async fn start_container(
        &self,
        image_name: &str,
        container_port: u16,
        cmd: Option<Vec<String>>,
        env: Option<Vec<String>>,
    ) -> Result<String> {
        let container_config = Config {
            image: Some(image_name.to_string()),
            cmd,
            env,
            exposed_ports: Some(
                [(
                    format!("{}/tcp", container_port),
                    std::collections::HashMap::new(),
                )]
                .into_iter()
                .collect(),
            ),
            host_config: Some(bollard::service::HostConfig {
                port_bindings: Some(
                    [(
                        format!("{}/tcp", container_port),
                        Some(vec![bollard::service::PortBinding {
                            host_ip: Some("127.0.0.1".to_string()),
                            host_port: Some("0".to_string()), // 0 means Docker assigns random available port
                        }]),
                    )]
                    .into_iter()
                    .collect(),
                ),
                ..Default::default()
            }),
            ..Default::default()
        };

        let container = self
            .docker
            .create_container::<String, String>(None, container_config)
            .await
            .context("Failed to create container")?;

        self.docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .context("Failed to start container")?;

        Ok(container.id)
    }

    /// Get the dynamically assigned host port for a container
    /// Returns the host port that maps to the given container port
    pub async fn get_host_port(&self, container_id: &str, container_port: u16) -> Result<u16> {
        let inspect = self
            .docker
            .inspect_container(container_id, None)
            .await
            .context("Failed to inspect container")?;

        let port_key = format!("{}/tcp", container_port);
        let host_port = inspect
            .network_settings
            .and_then(|ns| ns.ports)
            .and_then(|ports| ports.get(&port_key).cloned())
            .and_then(|bindings| bindings)
            .and_then(|bindings| bindings.first().cloned())
            .and_then(|binding| binding.host_port)
            .context("Failed to get host port from container")?;

        host_port
            .parse::<u16>()
            .context("Failed to parse host port as u16")
    }

    /// Wait for a port to become accessible with timeout
    pub async fn wait_for_port(
        &self,
        container_id: &str,
        port: u16,
        timeout_duration: Duration,
    ) -> Result<()> {
        let check = async {
            loop {
                // Try to connect to the port
                if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
                    .await
                    .is_ok()
                {
                    return Ok(());
                }

                // Check if container is still running
                let inspect = self.docker.inspect_container(container_id, None).await?;
                if inspect.state.and_then(|s| s.running) != Some(true) {
                    anyhow::bail!("Container stopped before port became accessible");
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };

        timeout(timeout_duration, check)
            .await
            .context("Timeout waiting for port")?
    }

    /// Perform HTTP health check with retries
    pub async fn http_health_check(
        &self,
        port: u16,
        path: &str,
        timeout_duration: Duration,
    ) -> Result<bool> {
        let url = format!("http://127.0.0.1:{}{}", port, path);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let check = async {
            loop {
                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => return Ok(true),
                    Ok(_) => return Ok(false), // Non-2xx status
                    Err(_) => {
                        // Connection error, retry
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        };

        timeout(timeout_duration, check).await.unwrap_or(Ok(false))
    }

    /// Stop and remove a container
    pub async fn cleanup_container(&self, container_id: &str) -> Result<()> {
        self.docker
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .context("Failed to remove container")?;
        Ok(())
    }

    /// Remove an image
    pub async fn cleanup_image(&self, image_name: &str) -> Result<()> {
        let _ = self.docker.remove_image(image_name, None, None).await;
        Ok(())
    }

    /// Get container logs
    pub async fn get_container_logs(&self, container_id: &str) -> Result<String> {
        let logs_options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            ..Default::default()
        };

        let mut log_stream = self.docker.logs(container_id, Some(logs_options));
        let mut output = String::new();

        while let Some(log) = log_stream.next().await {
            if let Ok(log_output) = log {
                output.push_str(&log_output.to_string());
            }
        }

        Ok(output)
    }
}
