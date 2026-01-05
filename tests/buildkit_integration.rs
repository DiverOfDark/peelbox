/// BuildKit Integration Tests
///
/// These tests verify the complete BuildKit gRPC workflow by building
/// a container image and validating its properties.
///
/// Requirements:
/// - Docker or Podman must be installed and running
/// - BuildKit support (enabled by default in Docker 23.0+ and Podman 4.0+)
///
/// The tests use testcontainers to automatically manage BuildKit containers
/// and connect to them via the BuildKit gRPC protocol (no buildctl required).
///
/// Implementation Status:
/// - ✅ Core tests enabled (FileSync, Session, LLB submission)
/// - ⏸️ Output format tests still ignored (Phase 7 not implemented)
///
/// Usage:
///   cargo test --test buildkit_integration -- --nocapture
mod support;

use anyhow::{Context, Result};
use bollard::container::{
    Config, LogsOptions, RemoveContainerOptions, StartContainerOptions, WaitContainerOptions,
};
use bollard::Docker;
use futures_util::stream::StreamExt;
use serial_test::serial;
use support::ContainerTestHarness;

/// Shared test fixture: Build peelbox image using BuildKit
/// Returns (image_name, docker_client)
async fn build_peelbox_image(test_name: &str) -> Result<(String, Docker)> {
    let harness = ContainerTestHarness::new()?;

    let spec_path = std::env::current_dir()
        .context("Failed to get current directory")?
        .join("universalbuild.json");

    let context_path = std::env::current_dir().context("Failed to get current directory")?;

    let image_name = format!("localhost/peelbox-test-{}:latest", test_name);

    harness
        .build_image(&spec_path, &context_path, &image_name)
        .await?;

    let docker =
        Docker::connect_with_local_defaults().context("Failed to connect to Docker/Podman")?;

    Ok((image_name, docker))
}

/// Test that the image builds successfully and exists in registry
#[tokio::test]
#[serial]
async fn test_image_builds_successfully() -> Result<()> {
    println!("=== Image Build Test ===\n");

    let (image_name, docker) = build_peelbox_image("build").await?;

    let inspect = docker
        .inspect_image(&image_name)
        .await
        .context("Failed to inspect image")?;

    assert!(inspect.id.is_some(), "Image should have an ID");
    println!("✓ Image built successfully: {}", image_name);

    Ok(())
}

/// Test that the built image runs and outputs help text correctly
#[tokio::test]
#[serial]
async fn test_image_runs_help_command() -> Result<()> {
    println!("=== Image Execution Test ===\n");

    let (image_name, docker) = build_peelbox_image("help").await?;

    let container_config = Config {
        image: Some(image_name.clone()),
        cmd: Some(vec![
            "/usr/local/bin/peelbox".to_string(),
            "--help".to_string(),
        ]),
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

    let wait_result = docker
        .wait_container(&test_container.id, None::<WaitContainerOptions<String>>)
        .next()
        .await
        .context("No wait result")??;

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

    docker
        .remove_container(
            &test_container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await?;

    assert_eq!(
        wait_result.status_code, 0,
        "Container should exit successfully"
    );
    assert!(
        output.contains("peelbox"),
        "Help output should contain project name"
    );
    assert!(
        output.contains("Commands:") || output.contains("Usage:"),
        "Help output should show available commands"
    );

    println!("✓ Image runs successfully and outputs help");

    Ok(())
}

/// Test distroless layer structure: 2 layers, no wolfi-base, clean metadata
#[tokio::test]
#[serial]
async fn test_distroless_layer_structure() -> Result<()> {
    println!("=== Distroless Layer Structure Test ===\n");

    let (image_name, docker) = build_peelbox_image("layers").await?;

    let history = docker
        .image_history(&image_name)
        .await
        .context("Failed to get image history")?;

    // Verify no wolfi-base in layer history (proves squashing worked)
    for layer in &history {
        assert!(
            !layer.created_by.contains("wolfi-base"),
            "Found wolfi-base in layer history: {}. Squashing failed!",
            layer.created_by
        );
    }
    println!("✓ No wolfi-base in layer history (truly distroless)");

    // Count only OUR layers (identified by ": peelbox" prefix)
    let our_layers: Vec<_> = history
        .iter()
        .filter(|layer| layer.size > 0 && layer.created_by.contains(": peelbox"))
        .collect();

    println!("Image has {} peelbox layers:", our_layers.len());
    for (i, layer) in our_layers.iter().enumerate() {
        println!(
            "  Layer {}: {} bytes - {}",
            i + 1,
            layer.size,
            &layer.created_by
        );
    }

    assert_eq!(
        our_layers.len(),
        2,
        "Distroless image should have exactly 2 peelbox layers (runtime + app), found {}",
        our_layers.len()
    );
    println!("✓ Exactly 2 peelbox layers (runtime + app)");

    // Verify clean layer metadata format (': peelbox <name>')
    let runtime_layer = history
        .iter()
        .find(|l| l.created_by.contains("runtime"))
        .expect("Runtime layer should exist");
    assert!(
        runtime_layer.created_by.contains(": peelbox"),
        "Runtime layer should have ': peelbox' prefix, got: {}",
        runtime_layer.created_by
    );

    let app_layer = history
        .iter()
        .find(|l| l.created_by.contains("application"))
        .expect("Application layer should exist");
    assert!(
        app_layer.created_by.contains(": peelbox"),
        "Application layer should have ': peelbox' prefix, got: {}",
        app_layer.created_by
    );
    println!("✓ Clean layer metadata (': peelbox' prefix)");

    Ok(())
}

/// Test that image size is optimized for distroless
#[tokio::test]
#[serial]
async fn test_image_size_optimized() -> Result<()> {
    println!("=== Image Size Optimization Test ===\n");

    let (image_name, docker) = build_peelbox_image("size").await?;

    let inspect = docker
        .inspect_image(&image_name)
        .await
        .context("Failed to inspect image")?;

    let size_bytes = inspect.size.unwrap_or(0);
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
    println!("Image size: {:.2} MB", size_mb);

    assert!(
        size_mb < 200.0,
        "Distroless image should be < 200MB, found {:.2}MB",
        size_mb
    );

    if size_mb > 30.0 {
        println!(
            "⚠ Warning: Image is {:.2}MB, larger than typical distroless (~10-30MB)",
            size_mb
        );
    } else {
        println!("✓ Image size is optimized ({:.2}MB)", size_mb);
    }

    Ok(())
}

/// Test that the application binary exists and is executable
#[tokio::test]
#[serial]
async fn test_binary_exists_and_executable() -> Result<()> {
    println!("=== Binary Location Test ===\n");

    let (image_name, docker) = build_peelbox_image("binary").await?;

    // Run the binary with --version to verify it exists and executes
    let container_config = Config {
        image: Some(image_name.clone()),
        cmd: Some(vec![
            "/usr/local/bin/peelbox".to_string(),
            "--version".to_string(),
        ]),
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

    let wait_result = docker
        .wait_container(&test_container.id, None::<WaitContainerOptions<String>>)
        .next()
        .await
        .context("No wait result")??;

    docker
        .remove_container(
            &test_container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await?;

    assert_eq!(
        wait_result.status_code, 0,
        "Binary at /usr/local/bin/peelbox should exist and be executable"
    );
    println!("✓ Binary exists at /usr/local/bin/peelbox and is executable");

    Ok(())
}

/// Test various output types (OCI and Docker tarballs)
/// NOTE: This test currently uses peelbox build with --output flag
/// which is not yet fully implemented (Phase 7). The test will fail
/// until output format implementation is complete.
#[tokio::test]
#[serial]
#[ignore = "Output format implementation not complete (Phase 7)"]
async fn test_buildctl_output_types() -> Result<()> {
    println!("=== BuildKit Output Types Test ===\n");

    // Get the shared BuildKit container
    let (port, _container_id) = support::container_harness::get_buildkit_container().await?;

    let peelbox_binary = std::env::current_dir()?.join("target/release/peelbox");
    if !peelbox_binary.exists() {
        let build_status = std::process::Command::new("cargo")
            .args([
                "build",
                "--release",
                "--bin",
                "peelbox",
                "--no-default-features",
            ])
            .status()?;
        if !build_status.success() {
            anyhow::bail!("Failed to build peelbox binary");
        }
    }

    let spec_path = std::env::current_dir()?.join("universalbuild.json");
    let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

    // Test OCI tarball output
    println!("--- Testing OCI tarball output ---");
    let oci_dest = std::env::temp_dir().join("peelbox-test-oci.tar");

    let peelbox_oci = std::process::Command::new(&peelbox_binary)
        .args([
            "build",
            "--spec",
            spec_path.to_str().unwrap(),
            "--tag",
            "peelbox-test:latest",
            "--buildkit",
            &buildkit_addr,
            "--output",
            &format!("type=oci,dest={}", oci_dest.display()),
        ])
        .output()?;

    if !peelbox_oci.status.success() {
        eprintln!(
            "peelbox build (OCI) stderr:\n{}",
            String::from_utf8_lossy(&peelbox_oci.stderr)
        );
        anyhow::bail!("OCI tarball build failed");
    }

    assert!(oci_dest.exists(), "OCI tarball should be created");
    let oci_size = std::fs::metadata(&oci_dest)?.len();
    println!("✓ OCI tarball created: {} bytes", oci_size);
    std::fs::remove_file(&oci_dest)?;

    // Test Docker tarball output
    println!("\n--- Testing Docker tarball output ---");
    let docker_dest = std::env::temp_dir().join("peelbox-test-docker.tar");

    let peelbox_docker = std::process::Command::new(&peelbox_binary)
        .args([
            "build",
            "--spec",
            spec_path.to_str().unwrap(),
            "--tag",
            "peelbox-test:latest",
            "--buildkit",
            &buildkit_addr,
            "--output",
            &format!("type=docker,dest={}", docker_dest.display()),
        ])
        .output()?;

    if !peelbox_docker.status.success() {
        eprintln!(
            "peelbox build (Docker) stderr:\n{}",
            String::from_utf8_lossy(&peelbox_docker.stderr)
        );
        anyhow::bail!("Docker tarball build failed");
    }

    assert!(docker_dest.exists(), "Docker tarball should be created");
    let docker_size = std::fs::metadata(&docker_dest)?.len();
    println!("✓ Docker tarball created: {} bytes", docker_size);
    std::fs::remove_file(&docker_dest)?;

    println!("\n=== ✓ BuildKit Output Types Test PASSED ===");
    Ok(())
}
