use anyhow::{Context, Result};
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{
    LogsOptions, RemoveContainerOptions, StartContainerOptions, WaitContainerOptions,
};
use bollard::Docker;
use futures_util::stream::StreamExt;
use std::path::Path;
use std::time::Duration;

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
        service_name: Option<&str>,
        output_tar: Option<&Path>,
    ) -> Result<String> {
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

        let mut cmd = std::process::Command::new(&peelbox_binary);
        cmd.args([
            "build",
            "--spec",
            spec_path.to_str().unwrap(),
            "--tag",
            image_name,
            "--context",
            context_path.to_str().unwrap(),
        ]);

        if let Some(service) = service_name {
            cmd.args(["--service", service]);
        }

        if let Some(tar_path) = output_tar {
            cmd.args(["--output", tar_path.to_str().unwrap()]);
        } else {
            cmd.args(["--output", "type=docker"]);
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
            anyhow::bail!("peelbox build failed");
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
        container_port: Option<u16>,
        cmd: Option<Vec<String>>,
        env: Option<Vec<String>>,
    ) -> Result<String> {
        let (exposed_ports, host_config) = if let Some(port) = container_port {
            let ep = vec![format!("{}/tcp", port)];
            let mut pb = std::collections::HashMap::new();
            pb.insert(
                format!("{}/tcp", port),
                Some(vec![PortBinding {
                    host_ip: Some("127.0.0.1".to_string()),
                    host_port: Some("0".to_string()),
                }]),
            );
            (
                Some(ep),
                Some(HostConfig {
                    port_bindings: Some(pb),
                    ..Default::default()
                }),
            )
        } else {
            (None, None)
        };

        let container_config = ContainerCreateBody {
            image: Some(image_name.to_string()),
            cmd,
            env,
            exposed_ports,
            host_config,
            ..Default::default()
        };

        let container = self
            .docker
            .create_container(
                None::<bollard::query_parameters::CreateContainerOptions>,
                container_config,
            )
            .await
            .context("Failed to create container")?;

        self.docker
            .start_container(&container.id, None::<StartContainerOptions>)
            .await
            .context("Failed to start container")?;

        Ok(container.id)
    }

    pub async fn wait_for_exit(&self, container_id: &str, timeout: Duration) -> Result<i64> {
        let wait_fut = async {
            // First check if the container has already exited (handles fast-exiting containers
            // where the process finishes before the wait call is made).
            if let Ok(exit_code) = self.inspect_exit_code(container_id).await {
                return Ok(exit_code);
            }

            let options = WaitContainerOptions {
                condition: "not-running".to_string(),
            };
            let mut stream = self.docker.wait_container(container_id, Some(options));
            match stream.next().await {
                Some(Ok(response)) => Ok(response.status_code),
                Some(Err(_)) | None => {
                    // Wait API can fail for fast-exiting containers -- fall back to inspect.
                    self.inspect_exit_code(container_id)
                        .await
                        .context("Error waiting for container (wait API failed, inspect fallback also failed)")
                }
            }
        };

        tokio::time::timeout(timeout, wait_fut)
            .await
            .context("Timeout waiting for container to exit")?
    }

    /// Inspect a container and return its exit code if it has already stopped.
    async fn inspect_exit_code(&self, container_id: &str) -> Result<i64> {
        let inspect = self
            .docker
            .inspect_container(container_id, None)
            .await
            .context("Failed to inspect container")?;
        if let Some(state) = &inspect.state {
            if let Some(running) = state.running {
                if !running {
                    return Ok(state.exit_code.unwrap_or(0));
                }
            }
        }
        anyhow::bail!("Container is still running")
    }

    pub async fn get_host_port(&self, container_id: &str, container_port: u16) -> Result<u16> {
        let port_key = format!("{}/tcp", container_port);

        // Retry: port bindings may not be immediately available after container start
        for attempt in 0..10 {
            let inspect = self
                .docker
                .inspect_container(container_id, None)
                .await
                .context("Failed to inspect container")?;

            if let Some(host_port) = inspect
                .network_settings
                .and_then(|ns| ns.ports)
                .and_then(|ports| ports.get(&port_key).cloned())
                .and_then(|bindings| bindings)
                .and_then(|bindings| bindings.first().cloned())
                .and_then(|binding| binding.host_port)
            {
                return host_port
                    .parse::<u16>()
                    .context("Failed to parse host port as u16");
            }

            if attempt < 9 {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        anyhow::bail!(
            "Failed to get host port from container after 10 attempts (port {})",
            container_port
        )
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
                    // 404 means the server is running but the route doesn't exist —
                    // still a valid health signal for apps with no root route.
                    Ok(response) if response.status().as_u16() == 404 => return Ok(true),
                    Ok(_) => {
                        // Retry on other non-success status (e.g., 503 during startup)
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
        let _ = self
            .docker
            .remove_image(
                image_name,
                None::<bollard::query_parameters::RemoveImageOptions>,
                None,
            )
            .await;
        Ok(())
    }

    pub async fn get_container_logs(&self, container_id: &str) -> Result<String> {
        let logs_options = LogsOptions {
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
