use anyhow::{Context, Result};
use bollard::container::Config;
use bollard::container::LogsOptions;
use bollard::container::{RemoveContainerOptions, StartContainerOptions, WaitContainerOptions};
use bollard::Docker;
use futures_util::StreamExt;
use serial_test::serial;
use std::path::Path;

mod support;
use support::container_harness::get_buildkit_container;

async fn get_or_build_peelbox_image() -> Result<String> {
    let peelbox_binary = support::get_peelbox_binary();
    let (port, _container_id) = get_buildkit_container().await?;
    let buildkit_addr = format!("tcp://127.0.0.1:{}", port);
    let unique_suffix = uuid::Uuid::new_v4().to_string();

    let temp_dir = std::env::temp_dir().join(format!("peelbox-itest-{}", unique_suffix));
    std::fs::create_dir_all(&temp_dir)?;
    let spec_path = temp_dir.join("universalbuild.json");
    let context_path = std::env::current_dir()?
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let spec = serde_json::json!({
        "version": "1.0",
        "metadata": { "project_name": format!("peelbox-itest-{}", unique_suffix) },
        "build": {
            "packages": ["rust-1.92", "build-base", "openssl-dev", "pkgconf", "protoc"],
            "commands": [
                "mkdir -p /build && cp -r /context/. /build && cd /build && cargo build --release --no-default-features -p peelbox-cli"
            ],
            "cache": ["/root/.cargo/registry", "/root/.cargo/git", "/build/target"]
        },
        "runtime": {
            "base_image": "cgr.dev/chainguard/glibc-dynamic:latest",
            "command": ["/usr/local/bin/peelbox", "--help"],
            "packages": ["glibc", "ca-certificates", "openssl"],
            "copy": [{"from": "/build/target/release/peelbox", "to": "/usr/local/bin/peelbox"}]
        }
    });
    std::fs::write(&spec_path, serde_json::to_string_pretty(&spec)?)?;

    let temp_cache_dir =
        std::env::temp_dir().join(format!("peelbox-cache-itest-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_cache_dir)?;

    let image_name = format!("localhost/peelbox-test:{}", unique_suffix);
    let mut cmd = std::process::Command::new(&peelbox_binary);
    cmd.args([
        "build",
        "--spec",
        spec_path.to_str().unwrap(),
        "--tag",
        &image_name,
        "--buildkit",
        &buildkit_addr,
        "--context",
        context_path.to_str().unwrap(),
        "--quiet",
    ]);
    cmd.env("PEELBOX_CACHE_DIR", temp_cache_dir.to_str().unwrap());

    let output = cmd.output()?;
    if !output.status.success() {
        anyhow::bail!("Build failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(image_name)
}

#[tokio::test]
async fn test_image_builds_successfully() -> Result<()> {
    println!("=== Image Build Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    println!("✓ Image built successfully: {}", image_name);
    Ok(())
}

async fn verify_image_content(tar_path: &Path, required_file: &str) -> Result<()> {
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

    let mut scanned_count = 0;
    let mut found_required = false;
    for layer_path in &layer_digests {
        let mut archive = Archive::new(&tar_data[..]);
        let mut found = false;
        for entry in archive.entries()? {
            let mut entry: tar::Entry<&[u8]> = entry?;
            if entry.path()?.to_string_lossy() == *layer_path {
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

                    if file_path_str.contains(required_file)
                        && !file_entry.header().entry_type().is_dir()
                    {
                        found_required = true;
                    }
                }
                scanned_count += 1;
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

    if !found_required {
        anyhow::bail!(
            "Could not find required file '{}' in any image layer",
            required_file
        );
    }

    println!(
        "✓ Successfully scanned {} active layers from manifest (found {})",
        scanned_count, required_file
    );
    Ok(())
}

#[tokio::test]
async fn test_distroless_layer_structure() -> Result<()> {
    println!("=== Distroless Layer Structure Test ===\n");

    let unique_suffix = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("peelbox-test-{}", unique_suffix));
    std::fs::create_dir_all(&temp_dir)?;
    let spec_path = temp_dir.join("universalbuild.json");
    let oci_dest = temp_dir.join("image.tar");

    let spec = serde_json::json!({
        "version": "1.0",
        "metadata": { "project_name": format!("peelbox-test-{}", unique_suffix) },
        "build": {
            "commands": ["echo 'hello' > /hello.txt"],
            "packages": []
        },
        "runtime": {
            "base_image": "cgr.dev/chainguard/glibc-dynamic:latest",
            "command": ["cat", "/hello.txt"],
            "packages": ["ca-certificates"],
            "copy": [{"from": "/hello.txt", "to": "/hello.txt"}]
        }
    });
    std::fs::write(&spec_path, serde_json::to_string_pretty(&spec)?)?;

    let peelbox_binary = support::get_peelbox_binary();
    let (port, _container_id) = get_buildkit_container().await?;
    let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

    let temp_cache_dir =
        std::env::temp_dir().join(format!("peelbox-cache-verify-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_cache_dir)?;

    let mut cmd = std::process::Command::new(&peelbox_binary);
    cmd.args([
        "build",
        "--spec",
        spec_path.to_str().unwrap(),
        "--tag",
        &format!("peelbox-test:verify-{}", unique_suffix),
        "--buildkit",
        &buildkit_addr,
        "--output",
        &format!("dest={}", oci_dest.display()),
    ]);
    cmd.env("PEELBOX_CACHE_DIR", temp_cache_dir.to_str().unwrap());

    let output = cmd.output()?;
    if !output.status.success() {
        anyhow::bail!("Build failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    verify_image_content(&oci_dest, "hello.txt").await?;
    println!("✓ VERIFIED: No apk binary found and hello.txt exists in active layers!");
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
    let container_config = Config {
        image: Some(image_name.clone()),
        user: Some("root".to_string()),
        cmd: Some(vec![
            "/usr/local/bin/peelbox".to_string(),
            "--version".to_string(),
        ]),
        ..Default::default()
    };
    let test_container = docker
        .create_container::<String, String>(None, container_config)
        .await?;
    docker
        .start_container(&test_container.id, None::<StartContainerOptions<String>>)
        .await?;
    let mut wait_stream =
        docker.wait_container(&test_container.id, None::<WaitContainerOptions<String>>);
    let wait_result = wait_stream.next().await.context("No wait result")??;

    if wait_result.status_code != 0 {
        let mut log_stream = docker.logs(
            &test_container.id,
            Some(LogsOptions::<String> {
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
    Ok(())
}

#[tokio::test]
async fn test_image_runs_help_command() -> Result<()> {
    println!("=== Image Execution Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    let docker = Docker::connect_with_local_defaults()?;
    let container_config = Config {
        image: Some(image_name.clone()),
        user: Some("root".to_string()),
        cmd: Some(vec![
            "/usr/local/bin/peelbox".to_string(),
            "--help".to_string(),
        ]),
        ..Default::default()
    };
    let test_container = docker
        .create_container::<String, String>(None, container_config)
        .await?;
    docker
        .start_container(&test_container.id, None::<StartContainerOptions<String>>)
        .await?;
    let mut wait_stream =
        docker.wait_container(&test_container.id, None::<WaitContainerOptions<String>>);
    let wait_result = wait_stream.next().await.context("No wait result")??;

    let mut log_stream = docker.logs(
        &test_container.id,
        Some(LogsOptions::<String> {
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
    let unique_suffix = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir().join(format!("peelbox-output-{}", unique_suffix));
    std::fs::create_dir_all(&temp_dir)?;
    let spec_path = temp_dir.join("spec.json");
    std::fs::write(
        &spec_path,
        serde_json::to_string(&serde_json::json!({
            "version": "1.0", "metadata": { "project_name": format!("test-{}", unique_suffix) },
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
            &format!("test:oci-{}", unique_suffix),
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
            &format!("test:docker-{}", unique_suffix),
            "--buildkit",
            &buildkit_addr,
            "--output",
            &format!("dest={}", docker_dest.display()),
        ])
        .output()?;
    assert!(docker_dest.exists());
    Ok(())
}
