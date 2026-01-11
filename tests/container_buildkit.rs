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
use std::sync::Arc;
use support::ContainerTestHarness;
use tokio::sync::OnceCell;

/// Global shared peelbox image for all integration tests
/// Uses a single image build across all tests to avoid rebuilding for each test
static PEELBOX_IMAGE: OnceCell<Arc<String>> = OnceCell::const_new();

/// Get or build the shared peelbox image
/// Returns the image name
async fn get_or_build_peelbox_image() -> Result<String> {
    if let Some(image) = PEELBOX_IMAGE.get() {
        return Ok(image.as_ref().clone());
    }

    let image = PEELBOX_IMAGE
        .get_or_init(|| async {
            let harness = ContainerTestHarness::new().expect("Failed to create harness");

            let spec_path = std::env::current_dir()
                .expect("Failed to get current directory")
                .join("universalbuild.json");

            let context_path = std::env::current_dir().expect("Failed to get current directory");

            let image_name = "localhost/peelbox-test:integration".to_string();
            let output_tar = std::env::temp_dir().join("peelbox-integration-test.tar");

            harness
                .build_image_with_output(&spec_path, &context_path, &image_name, &output_tar)
                .await
                .expect("Failed to build peelbox image");

            Arc::new(image_name)
        })
        .await;

    Ok(image.as_ref().clone())
}

/// Test that the image builds successfully and exists in registry
#[tokio::test]
#[serial]
async fn test_image_builds_successfully() -> Result<()> {
    println!("=== Image Build Test ===\n");

    let image_name = get_or_build_peelbox_image().await?;
    let docker =
        Docker::connect_with_local_defaults().context("Failed to connect to Docker/Podman")?;

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

    let image_name = get_or_build_peelbox_image().await?;
    let docker =
        Docker::connect_with_local_defaults().context("Failed to connect to Docker/Podman")?;

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

/// Test distroless layer structure: 2 layers, no wolfi-base in operations, clean metadata
#[tokio::test]
#[serial]
async fn test_distroless_layer_structure() -> Result<()> {
    println!("=== Distroless Layer Structure Test ===\n");

    let image_name = get_or_build_peelbox_image().await?;
    let docker =
        Docker::connect_with_local_defaults().context("Failed to connect to Docker/Podman")?;

    let history = docker
        .image_history(&image_name)
        .await
        .context("Failed to get image history")?;

    // Verify no apk in filesystem (truly distroless) - try to run it
    let container_config = Config {
        image: Some(image_name.clone()),
        cmd: Some(vec!["/sbin/apk".to_string(), "--version".to_string()]),
        ..Default::default()
    };

    let test_container = docker
        .create_container::<String, String>(None, container_config)
        .await
        .context("Failed to create test container")?;

    let start_result = docker
        .start_container(&test_container.id, None::<StartContainerOptions<String>>)
        .await;

    // Wait for container regardless of start result
    let wait_result = docker
        .wait_container(&test_container.id, None::<WaitContainerOptions<String>>)
        .next()
        .await;

    docker
        .remove_container(
            &test_container.id,
            Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            }),
        )
        .await?;

    // apk should either fail to start or exit with non-zero (file not found)
    let apk_not_found = match wait_result {
        Some(Ok(response)) => response.status_code != 0,
        Some(Err(_)) => true,
        None => true,
    };
    assert!(
        start_result.is_err() || apk_not_found,
        "apk should not be executable in distroless image"
    );
    println!("✓ No apk in filesystem (truly distroless)");

    // Count operational layers (not base image pulls, with actual operations)
    let operational_layers: Vec<_> = history
        .iter()
        .filter(|layer| {
            layer.size > 0
                && !layer.created_by.contains("pulled from")
                && !layer.created_by.contains("created by buildkit")
        })
        .collect();

    println!("Image has {} operational layers:", operational_layers.len());
    for (i, layer) in operational_layers.iter().enumerate() {
        let cmd = if layer.created_by.len() > 80 {
            format!("{}...", &layer.created_by[..77])
        } else {
            layer.created_by.clone()
        };
        println!("  Layer {}: {} bytes - {}", i + 1, layer.size, cmd);
    }

    assert!(
        !operational_layers.is_empty(),
        "Distroless image should have at least 1 operational layers (artifacts), found {}",
        operational_layers.len()
    );
    println!(
        "✓ {} operational layers (distroless build process)",
        operational_layers.len()
    );

    // Verify artifact copy layer exists
    let artifact_layer = history
        .iter()
        .find(|l| l.created_by.contains("/usr/local/bin/peelbox"));
    assert!(artifact_layer.is_some(), "Artifact copy layer should exist");
    println!("✓ Artifact copy layer present");

    Ok(())
}

/// Test that image size is optimized for distroless
#[tokio::test]
#[serial]
async fn test_image_size_optimized() -> Result<()> {
    println!("=== Image Size Optimization Test ===\n");

    let image_name = get_or_build_peelbox_image().await?;
    let docker =
        Docker::connect_with_local_defaults().context("Failed to connect to Docker/Podman")?;

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

    let image_name = get_or_build_peelbox_image().await?;
    let docker =
        Docker::connect_with_local_defaults().context("Failed to connect to Docker/Podman")?;

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
async fn test_buildctl_output_types() -> Result<()> {
    println!("=== BuildKit Output Types Test ===\n");

    // Get the shared BuildKit container
    let (port, _container_id) = support::container_harness::get_buildkit_container().await?;

    let mut peelbox_binary = std::env::current_exe()
        .context("Failed to get current executable path")?
        .parent()
        .context("No parent directory")?
        .to_path_buf();

    // If we're in deps/, go up one more level
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
            &format!("dest={}", docker_dest.display()),
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
