use super::ContainerTestHarness;
use peelbox_core::output::schema::UniversalBuild;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

/// Finds the workspace root (where Cargo.lock lives) by walking up from CWD.
fn find_workspace_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join("Cargo.lock").exists() {
            return dir;
        }
        if !dir.pop() {
            // Fallback: use CARGO_MANIFEST_DIR and walk up
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
            let p = Path::new(&manifest_dir);
            return p
                .parent()
                .and_then(|p| p.parent())
                .unwrap_or(p)
                .to_path_buf();
        }
    }
}

/// Returns a shared Wolfi cache directory that persists across all E2E tests.
///
/// Copies `tests/data/APKINDEX.tar.gz` once (or when the source is newer),
/// so that the binary `packages.bin` cache is reused instead of re-parsed
/// for every single test invocation.
#[allow(dead_code)]
fn shared_wolfi_cache_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();

    DIR.get_or_init(|| {
        let mut target_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if !target_dir.join("Cargo.lock").exists() && !target_dir.join("target").exists() {
            if let Some(parent) = target_dir.parent() {
                if parent.join("Cargo.lock").exists() {
                    target_dir = parent.to_path_buf();
                } else if let Some(grandparent) = parent.parent() {
                    if grandparent.join("Cargo.lock").exists() {
                        target_dir = grandparent.to_path_buf();
                    }
                }
            }
        }

        let cache_dir = target_dir.join("target").join("test-wolfi-cache");
        let apkindex_dir = cache_dir.join("apkindex");
        std::fs::create_dir_all(&apkindex_dir).expect("Failed to create shared wolfi cache dir");

        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/APKINDEX.tar.gz");
        let dest = apkindex_dir.join("APKINDEX.tar.gz");

        let needs_copy = if dest.exists() {
            let src_mtime = std::fs::metadata(&src).and_then(|m| m.modified()).ok();
            let dest_mtime = std::fs::metadata(&dest).and_then(|m| m.modified()).ok();
            match (src_mtime, dest_mtime) {
                (Some(s), Some(d)) => s > d,
                _ => true,
            }
        } else {
            true
        };

        if needs_copy && src.exists() {
            std::fs::copy(&src, &dest).expect("Failed to copy APKINDEX.tar.gz to shared cache");
            // Invalidate the binary cache since the source changed
            let packages_bin = apkindex_dir.join("packages.bin");
            if packages_bin.exists() {
                std::fs::remove_file(&packages_bin).ok();
            }
        }

        cache_dir
    })
    .clone()
}

#[allow(dead_code)]
pub fn peelbox_bin() -> PathBuf {
    let mut path = env::current_exe()
        .expect("Failed to get current executable path")
        .parent()
        .expect("No parent")
        .parent()
        .expect("No parent")
        .to_path_buf();

    if path.ends_with("deps") {
        path = path.parent().expect("No parent").to_path_buf();
    }

    path.join("peelbox")
}

#[allow(dead_code)]
pub fn fixture_path(category: &str, name: &str) -> PathBuf {
    if category.starts_with("compat-") {
        find_workspace_root()
            .join("target")
            .join("compat-work")
            .join(category)
            .join(name)
    } else {
        PathBuf::from("tests/fixtures").join(category).join(name)
    }
}

#[allow(dead_code)]
pub fn load_expected(
    category: &str,
    fixture_name: &str,
    _mode: Option<&str>,
) -> Option<Vec<UniversalBuild>> {
    // Standard fixture path (relative to crate root)
    let expected_path = PathBuf::from("tests/fixtures")
        .join(category)
        .join(fixture_name)
        .join("universalbuild.json");

    // For compat fixtures (category like "compat-railpack"), the snapshot lives
    // in the assembled work directory under target/compat-work/.
    let resolved_path = if expected_path.exists() {
        expected_path
    } else if category.starts_with("compat-") {
        let compat_path = find_workspace_root()
            .join("target")
            .join("compat-work")
            .join(category)
            .join(fixture_name)
            .join("universalbuild.json");
        if compat_path.exists() {
            compat_path
        } else {
            return None;
        }
    } else {
        return None;
    };

    let content = std::fs::read_to_string(&resolved_path)
        .unwrap_or_else(|_| panic!("Failed to read expected JSON: {}", resolved_path.display()));

    match serde_json::from_str::<Vec<UniversalBuild>>(&content) {
        Ok(multi) => Some(multi),
        Err(e1) => match serde_json::from_str::<UniversalBuild>(&content) {
            Ok(single) => Some(vec![single]),
            Err(e2) => {
                panic!(
                        "Failed to parse expected JSON: {}\nAs Vec<UniversalBuild>: {}\nAs UniversalBuild: {}",
                        resolved_path.display(),
                        e1,
                        e2
                    );
            }
        },
    }
}

#[allow(dead_code)]
pub fn run_detection_with_mode(
    fixture: PathBuf,
    test_name: &str,
    mode: Option<&str>,
) -> Result<Vec<UniversalBuild>, String> {
    let temp_cache_dir = shared_wolfi_cache_dir();

    let git_dir = fixture.join(".git");
    if !git_dir.exists() {
        std::fs::create_dir_all(&git_dir).ok();
    }

    let mut cmd = Command::new(peelbox_bin());
    cmd.env("PEELBOX_PROVIDER", "embedded")
        .env("PEELBOX_MODEL_SIZE", "7B")
        .env("PEELBOX_ENABLE_RECORDING", "1")
        .env(
            "PEELBOX_RECORDING_MODE",
            std::env::var("PEELBOX_RECORDING_MODE").unwrap_or_else(|_| "auto".to_string()),
        )
        .env("PEELBOX_TEST_NAME", test_name)
        .env("PEELBOX_CACHE_DIR", temp_cache_dir.to_str().unwrap());

    if let Some(detection_mode) = mode {
        cmd.env("PEELBOX_DETECTION_MODE", detection_mode);
    }

    let output = cmd
        .arg("detect")
        .arg(fixture)
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to execute peelbox");

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        eprintln!("Peelbox stderr: {}", stderr);
    }

    if !output.status.success() {
        return Err(stderr.to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    if let Ok(results) = serde_json::from_str::<Vec<UniversalBuild>>(&stdout) {
        return Ok(results);
    }

    if let Ok(result) = serde_json::from_str::<UniversalBuild>(&stdout) {
        return Ok(vec![result]);
    }

    Err(format!("Failed to parse output as JSON: {}", stdout))
}

#[allow(dead_code)]
pub fn assert_detection_with_mode(
    results: &[UniversalBuild],
    category: &str,
    fixture_name: &str,
    mode: Option<&str>,
) {
    if mode == Some("llm") {
        assert!(!results.is_empty(), "Results should not be empty");
        assert!(
            !results[0].build.commands.is_empty(),
            "Build commands should not be empty"
        );
        eprintln!("Skipping universalbuild.json validation for LLM-only test (known to differ from deterministic detection)");
        return;
    }

    // Snapshot update mode: write detection output as the new snapshot
    if super::compat_discovery::should_update_snapshots() {
        if let Some(source) = super::compat_discovery::parse_compat_category(category) {
            let json = serde_json::to_string_pretty(results).expect("Failed to serialize results");
            super::compat_discovery::write_compat_snapshot(source, fixture_name, &json)
                .unwrap_or_else(|e| panic!("Failed to write compat snapshot: {}", e));
            eprintln!("Updated compat snapshot for {}/{}", source, fixture_name);
            return;
        }
        // Also update fixture snapshots when in update mode
        let fixture_snapshot = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(category)
            .join(fixture_name)
            .join("universalbuild.json");
        if fixture_snapshot.exists() {
            let json =
                serde_json::to_string_pretty(results).expect("Failed to serialize results");
            std::fs::write(&fixture_snapshot, format!("{}\n", json))
                .unwrap_or_else(|e| panic!("Failed to write fixture snapshot: {}", e));
            eprintln!("Updated fixture snapshot for {}/{}", category, fixture_name);
            return;
        }
    }

    let mut expected = load_expected(category, fixture_name, mode).unwrap_or_else(|| {
        panic!(
            "Expected JSON file not found for fixture '{}'. Expected file: tests/fixtures/{}/{}/universalbuild.json",
            fixture_name, category, fixture_name
        )
    });

    assert_eq!(
        results.len(),
        expected.len(),
        "Number of detected projects mismatch for {}",
        fixture_name
    );

    if expected.is_empty() {
        return;
    }

    assert!(
        !results[0].build.commands.is_empty(),
        "Build commands should not be empty"
    );

    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| a.metadata.project_name.cmp(&b.metadata.project_name));
    expected.sort_by(|a, b| a.metadata.project_name.cmp(&b.metadata.project_name));

    for (i, (detected, expected_build)) in sorted_results.iter().zip(expected.iter()).enumerate() {
        let project_name = detected
            .metadata
            .project_name
            .as_deref()
            .unwrap_or("<unknown>");

        // Validate complete UniversalBuild structure equality with pretty diff
        pretty_assertions::assert_eq!(
            detected,
            expected_build,
            "UniversalBuild mismatch for project '{}' at position {}",
            project_name,
            i
        );
    }
}

#[allow(dead_code)]
pub enum ContainerValidation {
    Port {
        port: u16,
        health_endpoint: Option<String>,
    },
    Stdout {
        expected_output: String,
    },
    /// Start container, poll logs until expected output appears, then kill.
    /// For apps that run indefinitely but aren't reachable via port (e.g., binds to 127.0.0.1).
    Log {
        expected_output: String,
    },
}

#[allow(dead_code)]
pub struct ContainerTestInfo {
    pub project_name: String,
    pub command: Vec<String>,
    pub env: Vec<String>,
    pub validation: ContainerValidation,
}

#[allow(dead_code)]
pub fn get_fixture_container_test_infos(
    category: &str,
    fixture_name: &str,
) -> Option<Vec<ContainerTestInfo>> {
    let fixture_dir = fixture_path(category, fixture_name);
    let spec_path = fixture_dir.join("universalbuild.json");

    if !spec_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&spec_path).ok()?;
    let ub: Vec<UniversalBuild> = serde_json::from_str(&content)
        .or_else(|_| serde_json::from_str::<UniversalBuild>(&content).map(|single| vec![single]))
        .ok()?;

    let expected_output_path = fixture_dir.join("expected_output.txt");
    let expected_log_path = fixture_dir.join("expected_log.txt");

    let infos: Vec<_> = ub
        .into_iter()
        .filter_map(|build| {
            let project_name = build
                .metadata
                .project_name
                .unwrap_or_else(|| "unknown".to_string());
            let command = build.runtime.command.clone();
            let env: Vec<String> = build
                .runtime
                .env
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();

            // Check for per-project expected output first (e.g.,
            // expected_output_api.txt), then fall back to the global
            // expected_output.txt. This lets monorepo fixtures declare
            // different expected stdout for each service.
            //
            // expected_log.txt is for apps that run indefinitely but
            // aren't reachable via port (e.g., bind to 127.0.0.1).
            // It polls logs while the container runs, then kills it.
            let per_project_path = fixture_dir.join(format!("expected_output_{}.txt", project_name));
            let validation = if per_project_path.exists() {
                let expected_output = std::fs::read_to_string(&per_project_path).ok()?;
                ContainerValidation::Stdout { expected_output }
            } else if expected_output_path.exists() {
                let expected_output = std::fs::read_to_string(&expected_output_path).ok()?;
                ContainerValidation::Stdout { expected_output }
            } else if expected_log_path.exists() {
                let expected_output = std::fs::read_to_string(&expected_log_path).ok()?;
                ContainerValidation::Log { expected_output }
            } else if !build.runtime.ports.is_empty() {
                let port = build.runtime.ports.first().copied()?;
                let health_endpoint = build.runtime.health.as_ref().map(|h| h.endpoint.clone());
                ContainerValidation::Port {
                    port,
                    health_endpoint,
                }
            } else {
                // No ports and no expected_output.txt — skip this entry
                return None;
            };

            Some(ContainerTestInfo {
                project_name,
                command,
                env,
                validation,
            })
        })
        .collect();

    if infos.is_empty() {
        return None;
    }

    Some(infos)
}

#[allow(dead_code)]
pub async fn run_container_integration_test(
    category: &str,
    fixture_name: &str,
) -> Result<(), String> {
    let fixture_path = fixture_path(category, fixture_name);
    let spec_path = fixture_path.join("universalbuild.json");

    if !spec_path.exists() {
        return Err(format!(
            "universalbuild.json not found for fixture {}",
            fixture_name
        ));
    }

    let infos = match get_fixture_container_test_infos(category, fixture_name) {
        Some(infos) => infos,
        None => {
            // No ports and no expected_output.txt — nothing to validate (e.g., background workers, Ecto-only apps)
            eprintln!(
                "Skipping container test for {} — no ports or expected output to validate",
                fixture_name
            );
            return Ok(());
        }
    };

    let harness =
        ContainerTestHarness::new().map_err(|e| format!("Failed to create harness: {}", e))?;

    for info in infos {
        let image_name = format!(
            "localhost/peelbox-test-{}-{}-{}:latest",
            category.replace("/", "-"),
            fixture_name,
            info.project_name.replace('@', "").replace('/', "-")
        );

        let image = harness
            .build_image(
                &spec_path,
                &fixture_path,
                &image_name,
                Some(&info.project_name),
                None,
            )
            .await
            .map_err(|e| format!("Failed to build image for {}: {}", info.project_name, e))?;

        match &info.validation {
            ContainerValidation::Port {
                port,
                health_endpoint,
            } => {
                let container_id = harness
                    .start_container(
                        &image,
                        Some(*port),
                        None,
                        if info.env.is_empty() {
                            None
                        } else {
                            Some(info.env.clone())
                        },
                    )
                    .await
                    .map_err(|e| {
                        format!("Failed to start container for {}: {}", info.project_name, e)
                    })?;

                let host_port = harness
                    .get_host_port(&container_id, *port)
                    .await
                    .map_err(|e| {
                        format!("Failed to get host port for {}: {}", info.project_name, e)
                    })?;

                let wait_result = harness
                    .wait_for_port(&container_id, host_port, Duration::from_secs(120))
                    .await;

                if wait_result.is_err() {
                    let logs = harness
                        .get_container_logs(&container_id)
                        .await
                        .unwrap_or_default();
                    let _ = harness.cleanup_container(&container_id).await;
                    let _ = harness.cleanup_image(&image_name).await;
                    return Err(format!(
                        "Container for {} failed to start on port {}: {:?}\nLogs:\n{}",
                        info.project_name, port, wait_result, logs
                    ));
                }

                if let Some(endpoint) = health_endpoint {
                    let health_ok = harness
                        .http_health_check(host_port, endpoint, Duration::from_secs(60))
                        .await
                        .map_err(|e| {
                            format!("Health check failed for {}: {}", info.project_name, e)
                        })?;

                    if !health_ok {
                        let logs = harness
                            .get_container_logs(&container_id)
                            .await
                            .unwrap_or_default();
                        let _ = harness.cleanup_container(&container_id).await;
                        let _ = harness.cleanup_image(&image_name).await;
                        return Err(format!(
                            "Health check for {} returned non-2xx status.\nContainer Logs:\n{}",
                            info.project_name, logs
                        ));
                    }
                }

                let _ = harness.cleanup_container(&container_id).await;
            }
            ContainerValidation::Stdout { expected_output } => {
                let container_id = harness
                    .start_container(
                        &image,
                        None,
                        None,
                        if info.env.is_empty() {
                            None
                        } else {
                            Some(info.env.clone())
                        },
                    )
                    .await
                    .map_err(|e| {
                        format!("Failed to start container for {}: {}", info.project_name, e)
                    })?;

                let exit_code = harness
                    .wait_for_exit(&container_id, Duration::from_secs(120))
                    .await
                    .map_err(|e| {
                        format!(
                            "Failed waiting for container exit for {}: {}",
                            info.project_name, e
                        )
                    })?;

                let logs = harness
                    .get_container_logs(&container_id)
                    .await
                    .map_err(|e| format!("Failed to get logs for {}: {}", info.project_name, e))?;

                let actual = logs.trim();
                let expected = expected_output.trim();

                // Use "contains" check to allow flexible matching (e.g.,
                // apps that include version numbers or decorative output).
                // We check output before exit code so that expected-failure
                // tests (e.g., apps that need a database) can match their
                // error output and pass even with non-zero exit codes.
                if !actual.contains(expected) {
                    let _ = harness.cleanup_container(&container_id).await;
                    let _ = harness.cleanup_image(&image_name).await;
                    if exit_code != 0 {
                        return Err(format!(
                            "Container for {} exited with code {} and stdout did not contain expected output.\n--- expected (substring) ---\n{}\n--- actual ---\n{}",
                            info.project_name, exit_code, expected, actual
                        ));
                    }
                    return Err(format!(
                        "Stdout mismatch for {}:\n--- expected (substring) ---\n{}\n--- actual ---\n{}",
                        info.project_name, expected, actual
                    ));
                }

                if exit_code != 0 {
                    eprintln!(
                        "note: container for {} exited with code {} but output matched expected text — treating as success",
                        info.project_name, exit_code
                    );
                }

                let _ = harness.cleanup_container(&container_id).await;
            }
            ContainerValidation::Log { expected_output } => {
                // Start container without port mapping — app may bind to
                // 127.0.0.1 or otherwise not be reachable from the host.
                let container_id = harness
                    .start_container(
                        &image,
                        None,
                        None,
                        if info.env.is_empty() {
                            None
                        } else {
                            Some(info.env.clone())
                        },
                    )
                    .await
                    .map_err(|e| {
                        format!("Failed to start container for {}: {}", info.project_name, e)
                    })?;

                let expected = expected_output.trim();

                // Poll container logs until expected output appears (up to 30s).
                let found = async {
                    for _ in 0..60 {
                        if let Ok(logs) = harness.get_container_logs(&container_id).await {
                            if logs.contains(expected) {
                                return true;
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    false
                }
                .await;

                if !found {
                    let logs = harness
                        .get_container_logs(&container_id)
                        .await
                        .unwrap_or_default();
                    let _ = harness.cleanup_container(&container_id).await;
                    let _ = harness.cleanup_image(&image_name).await;
                    return Err(format!(
                        "Log output for {} did not contain expected text within 30s.\n--- expected (substring) ---\n{}\n--- actual ---\n{}",
                        info.project_name, expected, logs.trim()
                    ));
                }

                let _ = harness.cleanup_container(&container_id).await;
            }
        }

        let _ = harness.cleanup_image(&image_name).await;
    }

    Ok(())
}
