/// Tests that peelbox build works with a standalone BuildKit daemon.
///
/// These tests require a separate BuildKit container (moby/buildkit:v0.12.5)
/// and Docker access.
use anyhow::{Context, Result};
use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{
    CreateImageOptions, LogsOptions, RemoveContainerOptions, StartContainerOptions,
    WaitContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;

mod support;

const BUILDKIT_CONTAINER_NAME: &str = "peelbox-test-buildkit";

type BuildKitContainerCell = Arc<Option<(u16, String)>>;

static BUILDKIT_CONTAINER: OnceCell<BuildKitContainerCell> = OnceCell::const_new();

/// Starts (or reuses) a standalone moby/buildkit container and returns (port, container_id).
async fn get_buildkit_container() -> Result<(u16, String)> {
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

        let image_name = "moby/buildkit:v0.12.5";
        if docker.inspect_image(image_name).await.is_err() {
            eprintln!("Pulling BuildKit image {}...", image_name);
            let mut pull_stream = docker.create_image(
                Some(CreateImageOptions {
                    from_image: Some(image_name.to_string()),
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

        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "1234/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some("0".to_string()),
            }]),
        );

        let exposed_ports = vec!["1234/tcp".to_string()];

        let config = ContainerCreateBody {
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
                Some(
                    bollard::query_parameters::CreateContainerOptionsBuilder::default()
                        .name(BUILDKIT_CONTAINER_NAME)
                        .build(),
                ),
                config,
            )
            .await;

        match create_result {
            Ok(container_response) => {
                let container_id = container_response.id;

                docker
                    .start_container(&container_id, None::<StartContainerOptions>)
                    .await
                    .context("Failed to start BuildKit container")?;

                tokio::time::sleep(Duration::from_secs(3)).await;

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

/// Returns the cargo target directory for test artifacts
fn get_cargo_target_dir() -> PathBuf {
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(target_dir);
    }

    let mut path = std::env::current_exe().expect("Failed to get current executable path");

    while let Some(parent) = path.parent() {
        if parent.file_name().and_then(|n| n.to_str()) == Some("target") {
            return parent.to_path_buf();
        }
        path = parent.to_path_buf();
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
}

async fn get_or_build_peelbox_image() -> Result<String> {
    let peelbox_binary = support::get_peelbox_binary();
    let (port, _container_id) = get_buildkit_container().await?;
    let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

    let context_path = std::env::current_dir()?
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let temp_cache_dir = support::get_test_temp_dir();

    // Use a unique tag per run so we never accidentally validate a stale image.
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let image_name = format!("localhost/peelbox-test:{}", run_id);
    let mut cmd = std::process::Command::new(&peelbox_binary);
    cmd.args([
        "build",
        "--spec",
        context_path.join("universalbuild.json").to_str().unwrap(),
        "--tag",
        &image_name,
        "--buildkit",
        &buildkit_addr,
        "--context",
        context_path.to_str().unwrap(),
        "--quiet",
    ]);
    cmd.env("PEELBOX_CACHE_DIR", temp_cache_dir.to_str().unwrap());
    cmd.env("PEELBOX_LOG_LEVEL", "debug");

    let output = cmd.output()?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("--- BUILD FAILURE DETAILS (get_or_build) ---");
        println!("STDOUT:\n{}", stdout);
        println!("STDERR:\n{}", stderr);
        println!("--------------------------------------------");
        anyhow::bail!("Build failed: {}", stderr);
    }

    Ok(image_name.to_string())
}

#[tokio::test]
async fn test_image_builds_successfully() -> Result<()> {
    println!("=== Image Build Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    println!("✓ Image built successfully: {}", image_name);
    Ok(())
}

async fn verify_image_content(tar_path: &Path) -> Result<()> {
    use serde_json::Value;
    use std::io::Read;
    use tar::Archive;

    let mut tar_data = Vec::new();
    std::fs::File::open(tar_path)?.read_to_end(&mut tar_data)?;

    let mut archive = Archive::new(&tar_data[..]);
    let mut manifest_content = Vec::new();
    for entry in archive.entries()? {
        let mut entry: tar::Entry<&[u8]> = entry?;
        if entry.path()?.to_string_lossy() == "manifest.json" {
            entry.read_to_end(&mut manifest_content)?;
            break;
        }
    }

    if manifest_content.is_empty() {
        let mut archive = Archive::new(&tar_data[..]);
        for entry in archive.entries()? {
            let mut entry: tar::Entry<&[u8]> = entry?;
            if entry.path()?.to_string_lossy() == "index.json" {
                entry.read_to_end(&mut manifest_content)?;
                break;
            }
        }
    }

    if manifest_content.is_empty() {
        anyhow::bail!("Could not find manifest.json or index.json in tarball");
    }

    let manifest: Value = serde_json::from_slice(&manifest_content)?;
    let mut layer_digests = Vec::new();

    if let Some(manifests) = manifest.as_array() {
        for m in manifests {
            if let Some(layers) = m.get("Layers").and_then(|l| l.as_array()) {
                for l in layers {
                    if let Some(s) = l.as_str() {
                        layer_digests.push(s.to_string());
                    }
                }
            }
        }
    } else if let Some(manifests) = manifest.get("manifests").and_then(|m| m.as_array()) {
        for m in manifests {
            if let Some(digest) = m.get("digest").and_then(|d| d.as_str()) {
                let digest_path = format!("blobs/{}", digest.replace(":", "/"));

                let mut archive = Archive::new(&tar_data[..]);
                let mut inner_manifest_content = Vec::new();
                for entry in archive.entries()? {
                    let mut entry: tar::Entry<&[u8]> = entry?;
                    if entry.path()?.to_string_lossy() == digest_path {
                        entry.read_to_end(&mut inner_manifest_content)?;
                        break;
                    }
                }

                if !inner_manifest_content.is_empty() {
                    let inner_manifest: Value = serde_json::from_slice(&inner_manifest_content)?;
                    if let Some(layers) = inner_manifest.get("layers").and_then(|l| l.as_array()) {
                        for l in layers {
                            if let Some(d) = l.get("digest").and_then(|d| d.as_str()) {
                                layer_digests.push(format!("blobs/{}", d.replace(":", "/")));
                            }
                        }
                    }
                }
            }
        }
    }

    if layer_digests.is_empty() {
        anyhow::bail!("No layers found in manifest: {}", manifest);
    }

    for layer_path in &layer_digests {
        let mut archive = Archive::new(&tar_data[..]);
        let mut found = false;
        for entry in archive.entries()? {
            let mut entry: tar::Entry<&[u8]> = entry?;
            let path = entry.path()?;
            let entry_path = path.to_str().unwrap();
            if entry_path == *layer_path {
                found = true;
                let mut layer_data = Vec::new();
                entry.read_to_end(&mut layer_data)?;

                let decompressed_data: Vec<u8> = if layer_data.starts_with(&[0x1f, 0x8b]) {
                    let mut decoder = flate2::read::GzDecoder::new(&layer_data[..]);
                    let mut buf = Vec::new();
                    if decoder.read_to_end(&mut buf).is_ok() {
                        buf
                    } else {
                        layer_data
                    }
                } else {
                    layer_data
                };

                let mut layer_archive = Archive::new(&decompressed_data[..]);
                for file_entry in layer_archive.entries()? {
                    let file_entry: tar::Entry<&[u8]> = file_entry?;
                    let file_path = file_entry.path()?;
                    let file_path_str = file_path.to_string_lossy();

                    if file_path_str == "sbin/apk"
                        || file_path_str == "usr/bin/apk"
                        || (file_path_str.ends_with("/apk")
                            && !file_entry.header().entry_type().is_dir())
                    {
                        let entry_type = if file_entry.header().entry_type().is_dir() {
                            "directory"
                        } else {
                            "file"
                        };
                        anyhow::bail!(
                            "Found forbidden {} '{}' in active layer {}",
                            entry_type,
                            file_path_str,
                            layer_path
                        );
                    }
                }
                break;
            }
        }
        if !found {
            println!(
                "  ⚠ Warning: Layer {} referenced in manifest not found in tarball",
                layer_path
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_distroless_layer_structure() -> Result<()> {
    println!("=== Distroless Layer Structure Test ===\n");

    let temp_dir = support::get_test_temp_dir();
    let oci_dest = temp_dir.join("distroless.tar");

    let context_path = std::env::current_dir()?
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let universal_spec = context_path.join("universalbuild.json");

    let peelbox_binary = support::get_peelbox_binary();
    let (port, _container_id) = get_buildkit_container().await?;
    let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

    let external_cache_dir = get_cargo_target_dir().join("peelbox-test-external-cache");
    std::fs::create_dir_all(&external_cache_dir)
        .context("Failed to create external cache directory")?;

    let mut cmd = std::process::Command::new(&peelbox_binary);
    cmd.args([
        "build",
        "--spec",
        universal_spec.to_str().unwrap(),
        "--tag",
        "peelbox-test:verify",
        "--buildkit",
        &buildkit_addr,
        "--output",
        &format!("dest={}", oci_dest.display()),
    ]);
    cmd.current_dir(context_path);
    cmd.env("PEELBOX_CACHE_DIR", temp_dir.to_str().unwrap());

    let output = cmd.output()?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("--- BUILD FAILURE DETAILS (distroless_layer) ---");
        println!("STDOUT:\n{}", stdout);
        println!("STDERR:\n{}", stderr);
        println!("------------------------------------------------");
        anyhow::bail!("Build failed: {}", stderr);
    }

    verify_image_content(&oci_dest).await?;
    println!("✓ VERIFIED: No apk binary found in active layers!");
    Ok(())
}

#[tokio::test]
async fn test_image_size_optimized() -> Result<()> {
    println!("=== Image Size Optimization Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    let docker = Docker::connect_with_local_defaults()?;
    let inspect = docker.inspect_image(&image_name).await?;
    let size_mb = inspect.size.unwrap_or(0) as f64 / (1024.0 * 1024.0);
    println!("Image size: {:.2} MB", size_mb);
    assert!(size_mb < 200.0);
    Ok(())
}

#[tokio::test]
async fn test_binary_exists_and_executable() -> Result<()> {
    println!("=== Binary Location Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    let docker = Docker::connect_with_local_defaults()?;
    let container_config = ContainerCreateBody {
        image: Some(image_name.clone()),
        user: Some("root".to_string()),
        cmd: Some(vec!["--version".to_string()]),
        ..Default::default()
    };
    let test_container = docker
        .create_container(
            None::<bollard::query_parameters::CreateContainerOptions>,
            container_config,
        )
        .await?;
    docker
        .start_container(&test_container.id, None::<StartContainerOptions>)
        .await?;
    let mut wait_stream = docker.wait_container(&test_container.id, None::<WaitContainerOptions>);
    let wait_result = wait_stream.next().await.context("No wait result")??;

    let mut log_stream = docker.logs(
        &test_container.id,
        Some(LogsOptions {
            stdout: true,
            stderr: true,
            ..Default::default()
        }),
    );
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
    assert_eq!(wait_result.status_code, 0);
    assert!(output.contains("peelbox"));
    Ok(())
}

#[tokio::test]
async fn test_image_runs_help_command() -> Result<()> {
    println!("=== Image Execution Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    let docker = Docker::connect_with_local_defaults()?;
    let container_config = ContainerCreateBody {
        image: Some(image_name.clone()),
        user: Some("root".to_string()),
        cmd: Some(vec!["--help".to_string()]),
        ..Default::default()
    };
    let test_container = docker
        .create_container(
            None::<bollard::query_parameters::CreateContainerOptions>,
            container_config,
        )
        .await?;
    docker
        .start_container(&test_container.id, None::<StartContainerOptions>)
        .await?;
    let mut wait_stream = docker.wait_container(&test_container.id, None::<WaitContainerOptions>);
    let wait_result = wait_stream.next().await.context("No wait result")??;

    let mut log_stream = docker.logs(
        &test_container.id,
        Some(LogsOptions {
            stdout: true,
            stderr: true,
            ..Default::default()
        }),
    );
    let mut output = String::new();
    while let Some(log) = log_stream.next().await {
        if let Ok(log_output) = log {
            output.push_str(&log_output.to_string());
        }
    }

    if wait_result.status_code != 0 {
        println!("Container output (fail): {}", output);
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
    assert_eq!(wait_result.status_code, 0);
    assert!(output.contains("peelbox"));
    Ok(())
}

#[tokio::test]
async fn test_buildctl_output_types() -> Result<()> {
    println!("=== BuildKit Output Types Test ===\n");
    let temp_dir = support::get_test_temp_dir();
    let spec_path = temp_dir.join("spec.json");
    std::fs::write(
        &spec_path,
        serde_json::to_string(&serde_json::json!({
            "version": "1.0", "metadata": { "project_name": "test" },
            "build": { "steps": [], "packages": [] },
            "runtime": { "base_image": "cgr.dev/chainguard/wolfi-base:latest", "command": ["ls"], "packages": [], "env": {} }
        }))?,
    )?;
    let oci_dest = temp_dir.join("oci.tar");
    let docker_dest = temp_dir.join("docker.tar");
    let peelbox_binary = support::get_peelbox_binary();
    let (port, _container_id) = get_buildkit_container().await?;
    let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

    std::process::Command::new(&peelbox_binary)
        .args([
            "build",
            "--spec",
            spec_path.to_str().unwrap(),
            "--tag",
            "test:oci",
            "--buildkit",
            &buildkit_addr,
            "--output",
            &format!("type=oci,dest={}", oci_dest.display()),
        ])
        .output()?;
    assert!(oci_dest.exists());
    std::process::Command::new(&peelbox_binary)
        .args([
            "build",
            "--spec",
            spec_path.to_str().unwrap(),
            "--tag",
            "test:docker",
            "--buildkit",
            &buildkit_addr,
            "--output",
            &format!("dest={}", docker_dest.display()),
        ])
        .output()?;
    assert!(docker_dest.exists());
    Ok(())
}
