/// BuildKit Integration Test
///
/// This test verifies the complete BuildKit frontend workflow:
/// 1. Generate LLB using aipack frontend
/// 2. Build container image using BuildKit
/// 3. Run the built image and verify output
///
/// Requirements:
/// - Docker or Podman must be installed and running
/// - BuildKit support (enabled by default in Docker 23.0+ and Podman 4.0+)
///
/// Usage:
///   cargo test --test buildkit_integration -- --ignored --nocapture
///
/// The test is marked as #[ignore] because it requires a container runtime,
/// which may not be available in all environments (CI, sandboxes, etc.)
///
/// TestContainers will automatically detect and use Docker or Podman.

use anyhow::{Context, Result};
use bollard::container::{Config, LogsOptions, RemoveContainerOptions, StartContainerOptions, WaitContainerOptions};
use bollard::Docker;
use futures_util::stream::StreamExt;
use std::io::Write;
use std::process::Stdio;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;
use testcontainers::{GenericImage, ImageExt};

#[tokio::test]
async fn test_buildkit_integration_aipack_build() -> Result<()> {
    println!("=== BuildKit Integration Test ===");
    println!("Requirements: Docker or Podman must be installed and running\n");

    // Step 1: Start BuildKit container using testcontainers
    println!("--- Step 1: Starting BuildKit container ---");

    let buildkit_image = GenericImage::new("moby/buildkit", "latest")
        .with_wait_for(WaitFor::message_on_stderr("running server on"))
        .with_privileged(true);
    // TODO: Add volume caching for /var/lib/buildkit to persist layers between runs

    let buildkit_container = buildkit_image.start().await?;
    let container_id = buildkit_container.id();

    println!("✓ BuildKit container running: {}", container_id);

    // Give BuildKit a moment to fully start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Step 2: Build aipack binary in release mode (if not already built)
    println!("\n--- Step 2: Building aipack binary ---");
    let aipack_binary = std::env::current_dir()
        .context("Failed to get current directory")?
        .join("target/release/aipack");

    if !aipack_binary.exists() {
        println!("Building aipack binary...");
        let build_status = std::process::Command::new("cargo")
            .args(&["build", "--release", "--bin", "aipack", "--no-default-features"])
            .status()
            .context("Failed to build aipack")?;

        if !build_status.success() {
            anyhow::bail!("Failed to build aipack binary");
        }
    }
    println!("✓ aipack binary available at: {}", aipack_binary.display());

    // Step 3: Run aipack frontend to generate LLB
    println!("\n--- Step 3: Generating LLB with aipack frontend ---");
    let spec_path = std::env::current_dir()
        .context("Failed to get current directory")?
        .join("universalbuild.json");

    let aipack_output = std::process::Command::new(&aipack_binary)
        .args(&["frontend", "--spec", spec_path.to_str().unwrap()])
        .output()
        .context("Failed to run aipack frontend")?;

    if !aipack_output.status.success() {
        anyhow::bail!(
            "aipack frontend failed: {}",
            String::from_utf8_lossy(&aipack_output.stderr)
        );
    }

    let llb_data = aipack_output.stdout;
    assert!(!llb_data.is_empty(), "LLB data should not be empty");
    println!("✓ Generated LLB: {} bytes", llb_data.len());

    // Step 4: Connect to Docker/Podman via bollard
    println!("\n--- Step 4: Connecting to container runtime ---");
    let docker = Docker::connect_with_local_defaults()
        .context("Failed to connect to Docker/Podman")?;
    println!("✓ Connected to container runtime");

    // Step 5: Build image using BuildKit via buildctl from host
    println!("\n--- Step 5: Building image with buildctl ---");
    let image_name = "localhost/aipack-test:latest";
    let repo_path = std::env::current_dir()
        .context("Failed to get current directory")?;

    println!("Running buildctl build...");
    let buildkit_addr = format!("docker-container://{}", container_id);

    let mut buildctl = std::process::Command::new("buildctl")
        .args(&[
            "--addr", &buildkit_addr,
            "build",
            "--progress=plain",
            "--local", &format!("context={}", repo_path.display()),
            "--output", &format!("type=docker,name={}", image_name),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn buildctl")?;

    // Write LLB to stdin
    if let Some(mut stdin) = buildctl.stdin.take() {
        stdin.write_all(&llb_data).context("Failed to write LLB to buildctl stdin")?;
    }

    let buildctl_output = buildctl.wait_with_output()
        .context("Failed to wait for buildctl")?;

    if !buildctl_output.status.success() {
        eprintln!("buildctl stdout:\n{}", String::from_utf8_lossy(&buildctl_output.stdout));
        eprintln!("buildctl stderr:\n{}", String::from_utf8_lossy(&buildctl_output.stderr));
        anyhow::bail!("buildctl failed");
    }

    // Load the image into local Docker/Podman
    let cli_cmd = if std::process::Command::new("docker").arg("--version").output().is_ok() {
        "docker"
    } else if std::process::Command::new("podman").arg("--version").output().is_ok() {
        "podman"
    } else {
        anyhow::bail!("Neither docker nor podman CLI found");
    };

    let mut docker_load = std::process::Command::new(cli_cmd)
        .args(&["load"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn docker load")?;

    if let Some(mut stdin) = docker_load.stdin.take() {
        stdin.write_all(&buildctl_output.stdout).context("Failed to write tar to docker load")?;
    }

    let load_output = docker_load.wait_with_output()
        .context("Failed to wait for docker load")?;

    if !load_output.status.success() {
        anyhow::bail!(
            "docker load failed: {}",
            String::from_utf8_lossy(&load_output.stderr)
        );
    }

    println!("✓ Image built and loaded: {}", String::from_utf8_lossy(&load_output.stdout).trim());

    // Step 6: Verify image exists
    println!("\n--- Step 6: Verifying image ---");
    let inspect = docker.inspect_image(image_name)
        .await
        .context("Failed to inspect image")?;

    assert!(!inspect.id.is_none(), "Image should have an ID");
    println!("✓ Image exists in local registry");

    // Step 7: Run the built image with --help flag
    println!("\n--- Step 7: Running image with --help ---");
    let container_config = Config {
        image: Some(image_name.to_string()),
        cmd: Some(vec!["/usr/local/bin/aipack".to_string(), "--help".to_string()]),
        ..Default::default()
    };

    let test_container = docker
        .create_container::<String, String>(None, container_config)
        .await
        .context("Failed to create test container")?;

    docker
        .start_container(&test_container.id, None::<StartContainerOptions<String>>)
        .await
        .context("Failed to start test container")?;

    // Wait for container to finish
    let _ = docker
        .wait_container(&test_container.id, None::<WaitContainerOptions<String>>)
        .next()
        .await
        .context("No wait result")??;

    // Get logs
    let logs_options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        ..Default::default()
    };

    let mut log_stream = docker.logs(&test_container.id, Some(logs_options));

    let mut output = String::new();
    while let Some(log) = log_stream.next().await {
        if let Ok(log_output) = log {
            output.push_str(&log_output.to_string());
        }
    }

    // Clean up test container
    docker
        .remove_container(
            &test_container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await
        .context("Failed to remove test container")?;

    // Step 8: Verify output contains expected help text
    println!("\n--- Step 8: Verifying output ---");
    println!("Help output:\n{}", output);

    assert!(
        output.contains("AI-powered buildkit frontend") || output.contains("aipack"),
        "Help output should contain project description"
    );
    assert!(
        output.contains("Commands:") || output.contains("USAGE:") || output.contains("Usage:"),
        "Help output should show available commands"
    );

    println!("✓ Help output is valid");

    // Cleanup: Remove test image
    println!("\n--- Cleanup ---");
    let _ = docker.remove_image(image_name, None, None).await;
    println!("✓ Test image removed");

    // BuildKit container will be automatically stopped and removed by testcontainers Drop
    println!("✓ BuildKit container will be cleaned up by testcontainers");

    println!("\n=== ✓ BuildKit Integration Test PASSED ===");
    Ok(())
}
