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

static mut CACHED_IMAGE_NAME: Option<String> = None;

async fn get_or_build_peelbox_image() -> Result<String> {
    unsafe {
        if let Some(ref name) = CACHED_IMAGE_NAME {
            return Ok(name.clone());
        }
    }

    let peelbox_binary = support::get_peelbox_binary();
    let (port, _container_id) = get_buildkit_container().await?;
    let buildkit_addr = format!("tcp://127.0.0.1:{}", port);

    let temp_dir = std::env::temp_dir().join(format!("peelbox-itest-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let spec_path = temp_dir.join("universalbuild.json");
    let context_path = std::env::current_dir()?;

    let spec = serde_json::json!({
        "version": "1.0",
        "metadata": { "project_name": "peelbox-itest" },
        "build": {
            "packages": ["rust-1.92", "build-base", "openssl-dev", "pkgconf", "protoc"],
            "commands": [
                "mkdir -p /build && cp -r /context/. /build && cd /build && cargo build --release --no-default-features"
            ],
            "cache": ["/root/.cargo/registry", "/root/.cargo/git", "/build/target"]
        },
        "runtime": {
            "base_image": "cgr.dev/chainguard/glibc-dynamic:latest",
            "command": ["/usr/local/bin/peelbox", "--help"],
            "packages": ["glibc", "ca-certificates"],
            "copy": [{"from": "/build/target/release/peelbox", "to": "/usr/local/bin/peelbox"}]
        }
    });
    std::fs::write(&spec_path, serde_json::to_string_pretty(&spec)?)?;

    let image_name = "localhost/peelbox-test:integration";
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
        "--quiet",
    ]);

    let output = cmd.output()?;
    if !output.status.success() {
        anyhow::bail!("Build failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    unsafe {
        CACHED_IMAGE_NAME = Some(image_name.to_string());
    }
    Ok(image_name.to_string())
}

#[tokio::test]
#[serial]
async fn test_image_builds_successfully() -> Result<()> {
    println!("=== Image Build Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    println!("✓ Image built successfully: {}", image_name);
    Ok(())
}

async fn verify_no_apk_in_tarball(tar_path: &Path) -> Result<()> {
    use std::io::Read;
    use tar::Archive;

    let file = std::fs::File::open(tar_path)?;
    let mut archive = Archive::new(file);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();

        if path.to_string_lossy().ends_with(".tar") || path.to_string_lossy().contains("layer.tar")
        {
            let mut layer_data = Vec::new();
            entry.read_to_end(&mut layer_data)?;

            let mut layer_archive = Archive::new(&layer_data[..]);
            for file_entry in layer_archive.entries()? {
                let file_entry = file_entry?;
                let file_path = file_entry.path()?;
                let file_path_str = file_path.to_string_lossy();

                if file_path_str.ends_with("/apk")
                    || file_path_str == "sbin/apk"
                    || file_path_str == "usr/bin/apk"
                {
                    anyhow::bail!(
                        "Found forbidden file '{}' in layer {:?}",
                        file_path_str,
                        path
                    );
                }
            }
        }
    }
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_distroless_layer_structure() -> Result<()> {
    println!("=== Distroless Layer Structure Test ===\n");

    let temp_dir = std::env::temp_dir().join(format!("peelbox-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let spec_path = temp_dir.join("universalbuild.json");
    let oci_dest = temp_dir.join("image.tar");

    let spec = serde_json::json!({
        "version": "1.0",
        "metadata": { "project_name": "peelbox-test" },
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

    let mut cmd = std::process::Command::new(&peelbox_binary);
    cmd.args([
        "build",
        "--spec",
        spec_path.to_str().unwrap(),
        "--tag",
        "peelbox-test:verify",
        "--buildkit",
        &buildkit_addr,
        "--output",
        &format!("dest={}", oci_dest.display()),
    ]);

    let output = cmd.output()?;
    if !output.status.success() {
        anyhow::bail!("Build failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    verify_no_apk_in_tarball(&oci_dest).await?;
    println!("✓ VERIFIED: No apk binary found in any layer content!");
    Ok(())
}

#[tokio::test]
#[serial]
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
#[serial]
async fn test_binary_exists_and_executable() -> Result<()> {
    println!("=== Binary Location Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    let docker = Docker::connect_with_local_defaults()?;
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
        .await?;
    docker
        .start_container(&test_container.id, None::<StartContainerOptions<String>>)
        .await?;
    let mut wait_stream =
        docker.wait_container(&test_container.id, None::<WaitContainerOptions<String>>);
    let wait_result = wait_stream.next().await.context("No wait result")??;
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
#[serial]
async fn test_image_runs_help_command() -> Result<()> {
    println!("=== Image Execution Test ===\n");
    let image_name = get_or_build_peelbox_image().await?;
    let docker = Docker::connect_with_local_defaults()?;
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
#[serial]
async fn test_buildctl_output_types() -> Result<()> {
    println!("=== BuildKit Output Types Test ===\n");
    let temp_dir = std::env::temp_dir().join(format!("peelbox-output-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
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
