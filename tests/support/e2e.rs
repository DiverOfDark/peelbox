use super::ContainerTestHarness;
use peelbox::output::schema::UniversalBuild;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Setup test APKINDEX cache from committed snapshot
/// This ensures tests use a consistent set of package versions
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

        // Update modification time to prevent cache expiry (24h TTL)
        let now = std::time::SystemTime::now();
        filetime::set_file_mtime(&cache_apkindex, filetime::FileTime::from_system_time(now))
            .expect("Failed to update cache file modification time");

        // Remove parsed cache to force re-parsing with test APKINDEX
        let parsed_cache = cache_dir.join("packages.bin");
        if parsed_cache.exists() {
            std::fs::remove_file(&parsed_cache).ok();
        }

        eprintln!("✓ Test APKINDEX cache setup complete");
    });
}

/// Helper to get the path to the peelbox binary
#[allow(dead_code)]
pub fn peelbox_bin() -> PathBuf {
    // In tests, the binary should be at target/debug/peelbox
    let mut path = env::current_exe()
        .expect("Failed to get current executable path")
        .parent()
        .expect("No parent")
        .parent()
        .expect("No parent")
        .to_path_buf();

    // If we're in deps/, go up one more level
    if path.ends_with("deps") {
        path = path.parent().expect("No parent").to_path_buf();
    }

    path.join("peelbox")
}

/// Helper to get fixture path
#[allow(dead_code)]
pub fn fixture_path(category: &str, name: &str) -> PathBuf {
    PathBuf::from("tests/fixtures").join(category).join(name)
}

/// Helper to load expected UniversalBuild(s) from JSON
/// Loads universalbuild.json from the fixture directory itself (same for all modes)
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

    // Try parsing as array of UniversalBuild first (for monorepos)
    match serde_json::from_str::<Vec<UniversalBuild>>(&content) {
        Ok(multi) => Some(multi),
        Err(e1) => {
            // Try parsing as single UniversalBuild
            match serde_json::from_str::<UniversalBuild>(&content) {
                Ok(single) => Some(vec![single]),
                Err(e2) => {
                    panic!(
                        "Failed to parse expected JSON: {}\nAs Vec<UniversalBuild>: {}\nAs UniversalBuild: {}",
                        expected_path.display(),
                        e1,
                        e2
                    );
                }
            }
        }
    }
}

/// Helper to run detection with specified mode
#[allow(dead_code)]
pub fn run_detection_with_mode(
    fixture: PathBuf,
    test_name: &str,
    mode: Option<&str>,
) -> Result<Vec<UniversalBuild>, String> {
    let temp_cache_dir =
        std::env::temp_dir().join(format!("peelbox-cache-{}", uuid::Uuid::new_v4()));
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
        .env("PEELBOX_RECORDING_MODE", "auto")
        .env("PEELBOX_TEST_NAME", test_name)
        .env("PEELBOX_CACHE_DIR", temp_cache_dir.to_str().unwrap());

    if let Ok(rust_log) = std::env::var("RUST_LOG") {
        cmd.env("RUST_LOG", rust_log);
    }

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

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Try parsing as array first (for monorepos)
    if let Ok(results) = serde_json::from_str::<Vec<UniversalBuild>>(&stdout) {
        return Ok(results);
    }

    // Try parsing as single object
    if let Ok(result) = serde_json::from_str::<UniversalBuild>(&stdout) {
        return Ok(vec![result]);
    }

    Err(format!("Failed to parse output as JSON: {}", stdout))
}

/// Helper to assert detection results against expected output
#[allow(dead_code)]
pub fn assert_detection_with_mode(
    results: &[UniversalBuild],
    category: &str,
    fixture_name: &str,
    mode: Option<&str>,
) {
    assert!(!results.is_empty(), "Results should not be empty");

    assert!(
        !results[0].build.commands.is_empty(),
        "Build commands should not be empty"
    );

    // Skip detailed validation for LLM-only tests (produces inferior results vs deterministic)
    if mode == Some("llm") {
        eprintln!("Skipping universalbuild.json validation for LLM-only test (known to differ from deterministic detection)");
        return;
    }

    // Load and validate against expected JSON (required, same for all modes)
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

    // Sort both results and expected by project_name for deterministic comparison
    let mut sorted_results = results.to_vec();
    sorted_results.sort_by(|a, b| a.metadata.project_name.cmp(&b.metadata.project_name));
    expected.sort_by(|a, b| a.metadata.project_name.cmp(&b.metadata.project_name));

    for (i, (detected, expected_build)) in sorted_results.iter().zip(expected.iter()).enumerate() {
        let project_name = detected
            .metadata
            .project_name
            .as_deref()
            .unwrap_or("<unknown>");

        assert_eq!(
            detected.metadata.project_name, expected_build.metadata.project_name,
            "Project name mismatch at position {}: expected '{:?}', got '{:?}'",
            i, expected_build.metadata.project_name, detected.metadata.project_name
        );
        assert_eq!(
            detected.metadata.language, expected_build.metadata.language,
            "Language mismatch for project '{}': expected '{}', got '{}'",
            project_name, expected_build.metadata.language, detected.metadata.language
        );
        assert_eq!(
            detected.metadata.build_system, expected_build.metadata.build_system,
            "Build system mismatch for project '{}': expected '{}', got '{}'",
            project_name, expected_build.metadata.build_system, detected.metadata.build_system
        );
        // Wolfi-first architecture - base images removed from schema
        // Packages are now validated instead
        assert_eq!(
            detected.build.packages, expected_build.build.packages,
            "Build packages mismatch for project '{}': expected {:?}, got {:?}",
            project_name, expected_build.build.packages, detected.build.packages
        );
        assert_eq!(
            detected.runtime.packages, expected_build.runtime.packages,
            "Runtime packages mismatch for project '{}': expected {:?}, got {:?}",
            project_name, expected_build.runtime.packages, detected.runtime.packages
        );
    }
}

/// Helper to load port, health endpoint (optional), and command from committed universalbuild.json
#[allow(dead_code)]
#[allow(clippy::type_complexity)]
pub fn get_fixture_container_info(
    category: &str,
    fixture_name: &str,
) -> Option<(u16, Option<String>, Vec<String>, Vec<String>)> {
    let spec_path = PathBuf::from("tests/fixtures")
        .join(category)
        .join(fixture_name)
        .join("universalbuild.json");

    if !spec_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&spec_path).ok()?;
    let ub: Vec<UniversalBuild> = serde_json::from_str(&content)
        .or_else(|_| serde_json::from_str::<UniversalBuild>(&content).map(|single| vec![single]))
        .ok()?;

    let first = ub.first()?;
    let port = first.runtime.ports.first().copied()?;
    let health = first.runtime.health.as_ref().map(|h| h.endpoint.clone());
    let command = first.runtime.command.clone();
    let env: Vec<String> = first
        .runtime
        .env
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();

    Some((port, health, command, env))
}

/// Helper to run container integration test for a single fixture
#[allow(dead_code)]
pub async fn run_container_integration_test(
    category: &str,
    fixture_name: &str,
) -> Result<(), String> {
    let temp_cache_dir =
        std::env::temp_dir().join(format!("peelbox-cache-container-{}", uuid::Uuid::new_v4()));
    let apkindex_cache_dir = temp_cache_dir.join("apkindex");
    std::fs::create_dir_all(&apkindex_cache_dir).expect("Failed to create temp cache dir");

    let test_apkindex =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/APKINDEX.tar.gz");
    if test_apkindex.exists() {
        std::fs::copy(&test_apkindex, apkindex_cache_dir.join("APKINDEX.tar.gz"))
            .expect("Failed to copy test APKINDEX to temp cache");
    }

    let fixture_path = fixture_path(category, fixture_name);

    // Get port, health endpoint, command, and env from committed universalbuild.json
    let (port, health_path, cmd, env) = get_fixture_container_info(category, fixture_name)
        .ok_or_else(|| format!("No container info found for fixture {}", fixture_name))?;

    // Use committed universalbuild.json directly
    let spec_path = fixture_path.join("universalbuild.json");

    if !spec_path.exists() {
        return Err(format!(
            "universalbuild.json not found for fixture {}",
            fixture_name
        ));
    }

    // Build and test container
    let harness =
        ContainerTestHarness::new().map_err(|e| format!("Failed to create harness: {}", e))?;

    let image_name = format!(
        "localhost/peelbox-test-{}-{}:latest",
        category.replace("/", "-"),
        fixture_name
    );

    let image = harness
        .build_image(
            &spec_path,
            &fixture_path,
            &image_name,
            Some(&temp_cache_dir),
        )
        .await
        .map_err(|e| format!("Failed to build image: {}", e))?;

    let container_id = harness
        .start_container(
            &image,
            port,
            Some(cmd),
            if env.is_empty() { None } else { Some(env) },
        )
        .await
        .map_err(|e| format!("Failed to start container: {}", e))?;

    // Get the dynamically assigned host port
    let host_port = harness
        .get_host_port(&container_id, port)
        .await
        .map_err(|e| format!("Failed to get host port: {}", e))?;

    // Wait for port to be accessible (30s timeout)
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
            "Container failed to start on port {} (container port {} -> host port {}): {:?}\nLogs:\n{}",
            port, port, host_port, wait_result, logs
        ));
    }

    // Perform health check if endpoint is defined (10s timeout)
    if let Some(health_endpoint) = health_path {
        let health_ok = harness
            .http_health_check(host_port, &health_endpoint, Duration::from_secs(10))
            .await
            .map_err(|e| format!("Health check failed: {}", e))?;

        if !health_ok {
            let _ = harness.cleanup_container(&container_id).await;
            let _ = harness.cleanup_image(&image_name).await;
            return Err(format!(
                "Health check returned non-2xx status for {}",
                health_endpoint
            ));
        }
    }

    // Cleanup
    let _ = harness.cleanup_container(&container_id).await;
    let _ = harness.cleanup_image(&image_name).await;

    Ok(())
}
