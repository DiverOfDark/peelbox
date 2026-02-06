use anyhow::{Context, Result};
use bollard::container::{Config, LogsOptions, RemoveContainerOptions, StartContainerOptions};
use bollard::service::{HostConfig, PortBinding};
use bollard::Docker;
use futures_util::stream::StreamExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;

type BuildKitContainerCell = Arc<Option<(u16, String)>>;

static BUILDKIT_CONTAINER: OnceCell<BuildKitContainerCell> = OnceCell::const_new();

const BUILDKIT_CONTAINER_NAME: &str = "peelbox-test-buildkit";

/// Returns the cargo target directory for test artifacts
fn get_cargo_target_dir() -> PathBuf {
    // Use CARGO_TARGET_DIR if set, otherwise find target dir from binary location
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(target_dir);
    }

    // Get the test binary's directory and navigate up to find target/
    let mut path = std::env::current_exe().expect("Failed to get current executable path");

    // Navigate up: target/debug/deps/test_binary -> target/
    while let Some(parent) = path.parent() {
        if parent.file_name().and_then(|n| n.to_str()) == Some("target") {
            return parent.to_path_buf();
        }
        path = parent.to_path_buf();
    }

    // Fallback to workspace root + target
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
}

pub async fn get_buildkit_container() -> Result<(u16, String)> {
    let docker = Docker::connect_with_local_defaults().context("Failed to connect to Docker")?;

    for attempt in 0..10 {
        if let Ok(inspect) = docker
            .inspect_container(BUILDKIT_CONTAINER_NAME, None)
            .await
        {
            if inspect.state.and_then(|s| s.running) == Some(true) {
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
            } else if attempt == 0 {
                let _ = docker
                    .remove_container(
                        BUILDKIT_CONTAINER_NAME,
                        Some(RemoveContainerOptions {
                            force: true,
                            ..Default::default()
                        }),
                    )
                    .await;
            }
        }

        if let Some(arc_opt) = BUILDKIT_CONTAINER.get() {
            if let Some(c) = arc_opt.as_ref() {
                return Ok((c.0, c.1.clone()));
            }
        }

        eprintln!("Starting BuildKit (ephemeral, cleaned on removal)");

        // Ensure the BuildKit image exists, pull if necessary
        let image_name = "moby/buildkit:v0.12.5";
        if docker.inspect_image(image_name).await.is_err() {
            eprintln!("Pulling BuildKit image {}...", image_name);
            use bollard::image::CreateImageOptions;
            let mut pull_stream = docker.create_image(
                Some(CreateImageOptions {
                    from_image: image_name,
                    ..Default::default()
                }),
                None,
                None,
            );
            while let Some(result) = pull_stream.next().await {
                result.context("Failed to pull BuildKit image")?;
            }
            eprintln!("BuildKit image pulled successfully");
        }

        // Create container without mounts - data written to container layer
        // Any anonymous volumes will be cleaned up when container is removed
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "1234/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some("0".to_string()),
            }]),
        );

        let mut exposed_ports = HashMap::new();
        exposed_ports.insert("1234/tcp".to_string(), HashMap::new());

        let config = Config {
            image: Some(image_name.to_string()),
            cmd: Some(vec!["--addr".to_string(), "tcp://0.0.0.0:1234".to_string()]),
            exposed_ports: Some(exposed_ports),
            host_config: Some(HostConfig {
                privileged: Some(true),
                port_bindings: Some(port_bindings),
                auto_remove: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };

        let create_result = docker
            .create_container(
                Some(bollard::container::CreateContainerOptions {
                    name: BUILDKIT_CONTAINER_NAME,
                    ..Default::default()
                }),
                config,
            )
            .await;

        match create_result {
            Ok(container_response) => {
                let container_id = container_response.id;

                docker
                    .start_container(&container_id, None::<StartContainerOptions<String>>)
                    .await
                    .context("Failed to start BuildKit container")?;

                // Wait for BuildKit to be ready
                tokio::time::sleep(Duration::from_secs(3)).await;

                // Get the mapped port
                let inspect = docker
                    .inspect_container(&container_id, None)
                    .await
                    .context("Failed to inspect BuildKit container")?;

                let port = inspect
                    .network_settings
                    .and_then(|ns| ns.ports)
                    .and_then(|ports| ports.get("1234/tcp").cloned())
                    .and_then(|bindings| bindings)
                    .and_then(|mut b| b.pop())
                    .and_then(|binding| binding.host_port)
                    .and_then(|port| port.parse::<u16>().ok())
                    .context("Failed to get BuildKit port")?;

                let _ = BUILDKIT_CONTAINER.set(Arc::new(Some((port, container_id.clone()))));

                return Ok((port, container_id));
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("409") || err_str.contains("Conflict") {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                anyhow::bail!("Failed to create BuildKit container: {}", e);
            }
        }
    }

    anyhow::bail!("Failed to obtain BuildKit container after multiple attempts");
}

pub struct ContainerTestHarness {
    docker: Docker,
}

#[allow(dead_code)]
impl ContainerTestHarness {
    pub fn new() -> Result<Self> {
        let docker =
            Docker::connect_with_local_defaults().context("Failed to connect to Docker/Podman")?;
        Ok(Self { docker })
    }

    pub async fn build_image(
        &self,
        spec_path: &Path,
        context_path: &Path,
        image_name: &str,
        cache_dir: &Path,
        service_name: Option<&str>,
        output_tar: Option<&Path>,
    ) -> Result<String> {
        let (port, _container_id) = get_buildkit_container().await?;

        let mut peelbox_binary = std::env::current_exe()
            .context("Failed to get current executable path")?
            .parent()
            .context("No parent directory")?
            .to_path_buf();

        if peelbox_binary.ends_with("deps") {
            peelbox_binary = peelbox_binary
                .parent()
                .context("No parent directory")?
                .to_path_buf();
        }

        let peelbox_binary = peelbox_binary.join("peelbox");

        if !peelbox_binary.exists() {
            anyhow::bail!("peelbox binary not found at {}", peelbox_binary.display());
        }

        let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

        // Set up external cache directory for test builds
        // Cache will be transferred over gRPC when BuildKit exports
        let external_cache_dir = if let Ok(custom_cache) = std::env::var("PEELBOX_TEST_CACHE_DIR") {
            PathBuf::from(custom_cache).join("external-cache")
        } else {
            get_cargo_target_dir().join("peelbox-test-external-cache")
        };
        std::fs::create_dir_all(&external_cache_dir)
            .context("Failed to create external cache directory")?;

        eprintln!(
            "Using external BuildKit cache at: {}",
            external_cache_dir.display()
        );

        let cache_path = format!("type=local,path={}", external_cache_dir.display());

        let mut cmd = std::process::Command::new(&peelbox_binary);
        cmd.args([
            "build",
            "--spec",
            spec_path.to_str().unwrap(),
            "--tag",
            image_name,
            "--buildkit",
            &buildkit_addr,
            "--context",
            context_path.to_str().unwrap(),
            "--cache",
            &cache_path,
        ]);

        if let Some(service) = service_name {
            cmd.args(["--service", service]);
        }

        if let Some(tar_path) = output_tar {
            cmd.args(["--output", tar_path.to_str().unwrap()]);
        } else {
            cmd.args(["--output", "type=docker"]);
        }

        // Use PEELBOX_CACHE_DIR env var for automatic caching
        // Prefer cache_dir parameter if provided, otherwise use external_cache_dir
        cmd.env("PEELBOX_CACHE_DIR", cache_dir);

        let peelbox_output = cmd.output().context("Failed to run peelbox build")?;

        if !peelbox_output.status.success() {
            eprintln!(
                "peelbox build stdout:\n{}",
                String::from_utf8_lossy(&peelbox_output.stdout)
            );
            eprintln!(
                "peelbox build stderr:\n{}",
                String::from_utf8_lossy(&peelbox_output.stderr)
            );
            anyhow::bail!("peelbox build failed");
        }

        self.docker
            .inspect_image(image_name)
            .await
            .context("Failed to inspect image after build - image may not have been loaded")?;

        Ok(image_name.to_string())
    }

    pub async fn build_image_with_output(
        &self,
        spec_path: &Path,
        context_path: &Path,
        image_name: &str,
        output_tar: &Path,
    ) -> Result<String> {
        let (port, _container_id) = get_buildkit_container().await?;

        let mut peelbox_binary = std::env::current_exe()
            .context("Failed to get current executable path")?
            .parent()
            .context("No parent directory")?
            .to_path_buf();

        if peelbox_binary.ends_with("deps") {
            peelbox_binary = peelbox_binary
                .parent()
                .context("No parent directory")?
                .to_path_buf();
        }

        let peelbox_binary = peelbox_binary.join("peelbox");

        if !peelbox_binary.exists() {
            anyhow::bail!("peelbox binary not found at {}", peelbox_binary.display());
        }

        let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

        // Set up external cache directory (same as build_image)
        let external_cache_dir = if let Ok(custom_cache) = std::env::var("PEELBOX_TEST_CACHE_DIR") {
            PathBuf::from(custom_cache).join("external-cache")
        } else {
            get_cargo_target_dir().join("peelbox-test-external-cache")
        };
        std::fs::create_dir_all(&external_cache_dir)
            .context("Failed to create external cache directory")?;

        eprintln!(
            "Using external BuildKit cache at: {}",
            external_cache_dir.display()
        );

        let cache_from_path = format!("type=local,src={}", external_cache_dir.display());
        let cache_to_path = format!("type=local,dest={}", external_cache_dir.display());

        let mut cmd = std::process::Command::new(&peelbox_binary);
        cmd.args([
            "build",
            "--spec",
            spec_path.to_str().unwrap(),
            "--tag",
            image_name,
            "--buildkit",
            &buildkit_addr,
            "--context",
            context_path.to_str().unwrap(),
            "--output",
            output_tar.to_str().unwrap(),
            "--quiet",
            "--cache-from",
            &cache_from_path,
            "--cache-to",
            &cache_to_path,
        ]);

        if let Ok(rust_log) = std::env::var("RUST_LOG") {
            cmd.env("RUST_LOG", rust_log);
        }

        let peelbox_output = cmd.output().context("Failed to run peelbox build")?;

        if !peelbox_output.status.success() {
            eprintln!(
                "peelbox build stdout:\n{}",
                String::from_utf8_lossy(&peelbox_output.stdout)
            );
            eprintln!(
                "peelbox build stderr:\n{}",
                String::from_utf8_lossy(&peelbox_output.stderr)
            );
        }

        let image_id = String::from_utf8_lossy(&peelbox_output.stdout)
            .trim()
            .to_string();

        let image_exists = if !image_id.is_empty() {
            self.docker.inspect_image(&image_id).await.is_ok()
        } else {
            false
        };

        if !image_exists {
            let load_output = std::process::Command::new("docker")
                .args(["load", "-i", output_tar.to_str().unwrap()])
                .output()
                .context("Failed to load image into Docker")?;

            if !load_output.status.success() {
                anyhow::bail!(
                    "Failed to load image into Docker: {}",
                    String::from_utf8_lossy(&load_output.stderr)
                );
            }
        } else {
            let _ = std::process::Command::new("docker")
                .args(["tag", &image_id, image_name])
                .status();
        }

        self.docker
            .inspect_image(image_name)
            .await
            .context("Failed to inspect image after build - image may not have been loaded")?;

        Ok(image_name.to_string())
    }

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
                            host_port: Some("0".to_string()),
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

    pub async fn wait_for_port(
        &self,
        container_id: &str,
        port: u16,
        timeout_duration: std::time::Duration,
    ) -> Result<()> {
        let check = async {
            loop {
                if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
                    .await
                    .is_ok()
                {
                    return Ok(());
                }

                let inspect = self.docker.inspect_container(container_id, None).await?;
                if inspect.state.and_then(|s| s.running) != Some(true) {
                    anyhow::bail!("Container stopped before port became accessible");
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };

        tokio::time::timeout(timeout_duration, check)
            .await
            .context("Timeout waiting for port")?
    }

    pub async fn http_health_check(
        &self,
        port: u16,
        path: &str,
        timeout_duration: std::time::Duration,
    ) -> Result<bool> {
        let url = format!("http://127.0.0.1:{}{}", port, path);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let check = async {
            loop {
                match client.get(&url).send().await {
                    Ok(response) if response.status().is_success() => return Ok(true),
                    Ok(_) => {
                        // Retry on non-success status (e.g., 503 during startup)
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                }
            }
        };

        tokio::time::timeout(timeout_duration, check)
            .await
            .unwrap_or(Ok(false))
    }

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

    pub async fn cleanup_image(&self, image_name: &str) -> Result<()> {
        let _ = self.docker.remove_image(image_name, None, None).await;
        Ok(())
    }

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
