use super::ContainerTestHarness;
use peelbox_core::output::schema::UniversalBuild;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[allow(dead_code)]
pub fn setup_test_apkindex_cache() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let test_apkindex =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/APKINDEX.tar.gz");

        if !test_apkindex.exists() {
            eprintln!("WARNING: Test APKINDEX not found at {:?}", test_apkindex);
            return;
        }

        let cache_dir = dirs::cache_dir()
            .expect("Failed to get cache dir")
            .join("peelbox")
            .join("apkindex");

        std::fs::create_dir_all(&cache_dir).expect("Failed to create cache dir");

        let cache_apkindex = cache_dir.join("APKINDEX.tar.gz");
        std::fs::copy(&test_apkindex, &cache_apkindex)
            .expect("Failed to copy test APKINDEX to cache");

        let now = std::time::SystemTime::now();
        filetime::set_file_mtime(&cache_apkindex, filetime::FileTime::from_system_time(now))
            .expect("Failed to update cache file modification time");

        let parsed_cache = cache_dir.join("packages.bin");
        if parsed_cache.exists() {
            std::fs::remove_file(&parsed_cache).ok();
        }

        eprintln!("✓ Test APKINDEX cache setup complete");
    });
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
    PathBuf::from("tests/fixtures").join(category).join(name)
}

#[allow(dead_code)]
pub fn load_expected(
    category: &str,
    fixture_name: &str,
    _mode: Option<&str>,
) -> Option<Vec<UniversalBuild>> {
    let expected_path = PathBuf::from("tests/fixtures")
        .join(category)
        .join(fixture_name)
        .join("universalbuild.json");

    if !expected_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("Failed to read expected JSON: {}", expected_path.display()));

    match serde_json::from_str::<Vec<UniversalBuild>>(&content) {
        Ok(multi) => Some(multi),
        Err(e1) => match serde_json::from_str::<UniversalBuild>(&content) {
            Ok(single) => Some(vec![single]),
            Err(e2) => {
                panic!(
                        "Failed to parse expected JSON: {}\nAs Vec<UniversalBuild>: {}\nAs UniversalBuild: {}",
                        expected_path.display(),
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
    let temp_cache_dir = super::get_test_temp_dir();

    let apkindex_cache_dir = temp_cache_dir.join("apkindex");
    std::fs::create_dir_all(&apkindex_cache_dir).expect("Failed to create temp cache dir");

    let test_apkindex =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/APKINDEX.tar.gz");
    if test_apkindex.exists() {
        std::fs::copy(&test_apkindex, apkindex_cache_dir.join("APKINDEX.tar.gz"))
            .expect("Failed to copy test APKINDEX to temp cache");
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

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
    let fixture_dir = PathBuf::from("tests/fixtures")
        .join(category)
        .join(fixture_name);
    let spec_path = fixture_dir.join("universalbuild.json");

    if !spec_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&spec_path).ok()?;
    let ub: Vec<UniversalBuild> = serde_json::from_str(&content)
        .or_else(|_| serde_json::from_str::<UniversalBuild>(&content).map(|single| vec![single]))
        .ok()?;

    let expected_output_path = fixture_dir.join("expected_output.txt");

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

            let validation = if !build.runtime.ports.is_empty() {
                let port = build.runtime.ports.first().copied()?;
                let health_endpoint =
                    build.runtime.health.as_ref().map(|h| h.endpoint.clone());
                ContainerValidation::Port {
                    port,
                    health_endpoint,
                }
            } else if expected_output_path.exists() {
                let expected_output =
                    std::fs::read_to_string(&expected_output_path).ok()?;
                ContainerValidation::Stdout { expected_output }
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
    let temp_cache_dir = super::get_test_temp_dir();

    let apkindex_cache_dir = temp_cache_dir.join("apkindex");
    std::fs::create_dir_all(&apkindex_cache_dir).expect("Failed to create temp cache dir");

    let test_apkindex =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/APKINDEX.tar.gz");
    if test_apkindex.exists() {
        std::fs::copy(&test_apkindex, apkindex_cache_dir.join("APKINDEX.tar.gz"))
            .expect("Failed to copy test APKINDEX to temp cache");
    }

    let fixture_path = fixture_path(category, fixture_name);
    let spec_path = fixture_path.join("universalbuild.json");

    if !spec_path.exists() {
        return Err(format!(
            "universalbuild.json not found for fixture {}",
            fixture_name
        ));
    }

    let infos = get_fixture_container_test_infos(category, fixture_name).ok_or_else(|| {
        format!(
            "No runnable container info found for fixture {}",
            fixture_name
        )
    })?;

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
                &temp_cache_dir,
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
                    .wait_for_port(&container_id, host_port, Duration::from_secs(30))
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
                        .http_health_check(host_port, endpoint, Duration::from_secs(10))
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
                    .wait_for_exit(&container_id, Duration::from_secs(30))
                    .await
                    .map_err(|e| {
                        format!(
                            "Failed waiting for container exit for {}: {}",
                            info.project_name, e
                        )
                    })?;

                if exit_code != 0 {
                    let logs = harness
                        .get_container_logs(&container_id)
                        .await
                        .unwrap_or_default();
                    let _ = harness.cleanup_container(&container_id).await;
                    let _ = harness.cleanup_image(&image_name).await;
                    return Err(format!(
                        "Container for {} exited with code {}\nLogs:\n{}",
                        info.project_name, exit_code, logs
                    ));
                }

                let logs = harness
                    .get_container_logs(&container_id)
                    .await
                    .map_err(|e| {
                        format!("Failed to get logs for {}: {}", info.project_name, e)
                    })?;

                let actual = logs.trim();
                let expected = expected_output.trim();

                if actual != expected {
                    let _ = harness.cleanup_container(&container_id).await;
                    let _ = harness.cleanup_image(&image_name).await;
                    return Err(format!(
                        "Stdout mismatch for {}:\n--- expected ---\n{}\n--- actual ---\n{}",
                        info.project_name, expected, actual
                    ));
                }

                let _ = harness.cleanup_container(&container_id).await;
            }
        }

        let _ = harness.cleanup_image(&image_name).await;
    }

    Ok(())
}
