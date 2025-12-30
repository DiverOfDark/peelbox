//! End-to-end tests using fixtures and binary execution
//!
//! These tests verify the complete detection pipeline by spawning the aipack binary:
//! - Bootstrap scanning
//! - LLM conversation with tool calling
//! - Validation of final output
//!
//! Tests use RecordingMode::Auto to replay cached LLM responses for deterministic testing.

mod support;

use aipack::output::schema::UniversalBuild;
use serial_test::serial;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use support::ContainerTestHarness;
use yare::parameterized;

/// Setup test APKINDEX cache from committed snapshot
/// This ensures tests use a consistent set of package versions
fn setup_test_apkindex_cache() {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let test_apkindex = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/APKINDEX.tar.gz");

        if !test_apkindex.exists() {
            eprintln!("WARNING: Test APKINDEX not found at {:?}", test_apkindex);
            return;
        }

        let cache_dir = dirs::cache_dir()
            .expect("Failed to get cache dir")
            .join("aipack")
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

/// Helper to get the path to the aipack binary
fn aipack_bin() -> PathBuf {
    // In tests, the binary should be at target/debug/aipack
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

    path.join("aipack")
}

/// Helper to get fixture path
fn fixture_path(category: &str, name: &str) -> PathBuf {
    PathBuf::from("tests/fixtures").join(category).join(name)
}

/// Helper to load expected UniversalBuild(s) from JSON
/// Loads universalbuild.json from the fixture directory itself (same for all modes)
fn load_expected(category: &str, fixture_name: &str, _mode: Option<&str>) -> Option<Vec<UniversalBuild>> {
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
        Ok(multi) => return Some(multi),
        Err(e1) => {
            // Try parsing as single UniversalBuild
            match serde_json::from_str::<UniversalBuild>(&content) {
                Ok(single) => return Some(vec![single]),
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
fn run_detection_with_mode(
    fixture: PathBuf,
    test_name: &str,
    mode: Option<&str>,
) -> Result<Vec<UniversalBuild>, String> {
    // Setup: Copy test APKINDEX snapshot to cache so tests use consistent package versions
    setup_test_apkindex_cache();

    // Create .git directory in fixture to prevent WalkBuilder from looking up the tree
    let git_dir = fixture.join(".git");
    if !git_dir.exists() {
        std::fs::create_dir_all(&git_dir).ok();
    }

    let mut cmd = Command::new(aipack_bin());
    cmd.env("AIPACK_PROVIDER", "embedded")
        .env("AIPACK_MODEL_SIZE", "7B")
        .env("AIPACK_ENABLE_RECORDING", "1")
        .env("AIPACK_RECORDING_MODE", "auto")
        .env("AIPACK_TEST_NAME", test_name);

    if let Some(detection_mode) = mode {
        cmd.env("AIPACK_DETECTION_MODE", detection_mode);
    }

    let output = cmd
        .arg("detect")
        .arg(fixture)
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to execute aipack");

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
fn assert_detection_with_mode(results: &[UniversalBuild], category: &str, fixture_name: &str, mode: Option<&str>) {
    assert!(!results.is_empty(), "Results should not be empty");

    assert!(
        !results[0].build.commands.is_empty(),
        "Build commands should not be empty"
    );

    assert!(
        results[0].metadata.confidence >= 0.5,
        "Confidence should be at least 0.5, got {}",
        results[0].metadata.confidence
    );

    // Load and validate against expected JSON (required, same for all modes)
    let mut expected = load_expected(category, fixture_name, mode).expect(&format!(
        "Expected JSON file not found for fixture '{}'. Expected file: tests/fixtures/{}/{}/universalbuild.json",
        fixture_name,
        category,
        fixture_name
    ));

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

//
// Parameterized E2E tests
//

// Single-language fixtures - all modes
#[parameterized(
    rust_cargo_full = { "rust-cargo", None },
    rust_cargo_llm = { "rust-cargo", Some("llm") },
    rust_cargo_static = { "rust-cargo", Some("static") },
    node_npm_full = { "node-npm", None },
    node_npm_llm = { "node-npm", Some("llm") },
    node_npm_static = { "node-npm", Some("static") },
    python_pip_full = { "python-pip", None },
    python_pip_llm = { "python-pip", Some("llm") },
    python_pip_static = { "python-pip", Some("static") },
    java_maven_full = { "java-maven", None },
    java_maven_llm = { "java-maven", Some("llm") },
    java_maven_static = { "java-maven", Some("static") },
    node_yarn_full = { "node-yarn", None },
    node_yarn_llm = { "node-yarn", Some("llm") },
    node_yarn_static = { "node-yarn", Some("static") },
    node_pnpm_full = { "node-pnpm", None },
    node_pnpm_llm = { "node-pnpm", Some("llm") },
    node_pnpm_static = { "node-pnpm", Some("static") },
    python_poetry_full = { "python-poetry", None },
    python_poetry_llm = { "python-poetry", Some("llm") },
    python_poetry_static = { "python-poetry", Some("static") },
    java_gradle_full = { "java-gradle", None },
    java_gradle_llm = { "java-gradle", Some("llm") },
    java_gradle_static = { "java-gradle", Some("static") },
    kotlin_gradle_full = { "kotlin-gradle", None },
    kotlin_gradle_llm = { "kotlin-gradle", Some("llm") },
    kotlin_gradle_static = { "kotlin-gradle", Some("static") },
    dotnet_csproj_full = { "dotnet-csproj", None },
    dotnet_csproj_llm = { "dotnet-csproj", Some("llm") },
    dotnet_csproj_static = { "dotnet-csproj", Some("static") },
    go_mod_full = { "go-mod", None },
    go_mod_llm = { "go-mod", Some("llm") },
    go_mod_static = { "go-mod", Some("static") },
    ruby_bundler_full = { "ruby-bundler", None },
    ruby_bundler_llm = { "ruby-bundler", Some("llm") },
    ruby_bundler_static = { "ruby-bundler", Some("static") },
    php_composer_full = { "php-composer", None },
    php_composer_llm = { "php-composer", Some("llm") },
    php_composer_static = { "php-composer", Some("static") },
    php_symfony_full = { "php-symfony", None },
    php_symfony_llm = { "php-symfony", Some("llm") },
    php_symfony_static = { "php-symfony", Some("static") },
    cpp_cmake_full = { "cpp-cmake", None },
    cpp_cmake_llm = { "cpp-cmake", Some("llm") },
    cpp_cmake_static = { "cpp-cmake", Some("static") },
    elixir_mix_full = { "elixir-mix", None },
    elixir_mix_llm = { "elixir-mix", Some("llm") },
    elixir_mix_static = { "elixir-mix", Some("static") },
    zig_build_llm = { "zig-build", Some("llm") },
    deno_fresh_llm = { "deno-fresh", Some("llm") },
)]
#[serial]
fn test_single_language(fixture_name: &str, mode: Option<&str>) {
    let fixture = fixture_path("single-language", fixture_name);
    let mode_suffix = mode.unwrap_or("detection");
    let test_name = format!(
        "e2e_test_{}_{}",
        fixture_name.replace("-", "_"),
        mode_suffix.replace("-", "_")
    );
    let results = run_detection_with_mode(fixture, &test_name, mode).expect("Detection failed");
    assert_detection_with_mode(&results, "single-language", fixture_name, mode);
}

// Monorepo fixtures - all modes
#[parameterized(
    npm_workspaces_full = { "npm-workspaces", None },
    npm_workspaces_llm = { "npm-workspaces", Some("llm") },
    npm_workspaces_static = { "npm-workspaces", Some("static") },
    cargo_workspace_full = { "cargo-workspace", None },
    cargo_workspace_llm = { "cargo-workspace", Some("llm") },
    cargo_workspace_static = { "cargo-workspace", Some("static") },
    turborepo_full = { "turborepo", None },
    turborepo_llm = { "turborepo", Some("llm") },
    turborepo_static = { "turborepo", Some("static") },
    gradle_multiproject_full = { "gradle-multiproject", None },
    gradle_multiproject_llm = { "gradle-multiproject", Some("llm") },
    gradle_multiproject_static = { "gradle-multiproject", Some("static") },
    maven_multimodule_full = { "maven-multimodule", None },
    maven_multimodule_llm = { "maven-multimodule", Some("llm") },
    maven_multimodule_static = { "maven-multimodule", Some("static") },
    polyglot_full = { "polyglot", None },
    polyglot_llm = { "polyglot", Some("llm") },
    polyglot_static = { "polyglot", Some("static") },
)]
#[serial]
fn test_monorepo(fixture_name: &str, mode: Option<&str>) {
    let fixture = fixture_path("monorepo", fixture_name);
    let mode_suffix = mode.unwrap_or("detection");
    let test_name = format!(
        "e2e_test_{}_{}",
        fixture_name.replace("-", "_"),
        mode_suffix.replace("-", "_")
    );
    let results = run_detection_with_mode(fixture, &test_name, mode).expect("Detection failed");
    assert_detection_with_mode(&results, "monorepo", fixture_name, mode);
}

// Edge-cases fixtures - LLM mode only for unknown technologies
#[parameterized(
    bazel_build_llm = { "bazel-build", Some("llm") },
)]
#[serial]
fn test_edge_cases(fixture_name: &str, mode: Option<&str>) {
    let fixture = fixture_path("edge-cases", fixture_name);
    let mode_suffix = mode.unwrap_or("detection");
    let test_name = format!(
        "e2e_test_{}_{}",
        fixture_name.replace("-", "_"),
        mode_suffix.replace("-", "_")
    );
    let results = run_detection_with_mode(fixture, &test_name, mode).expect("Detection failed");
    assert_detection_with_mode(&results, "edge-cases", fixture_name, mode);
}

//
// Container Integration Tests
//

/// Helper to load port, health endpoint (optional), and command from committed universalbuild.json
fn get_fixture_container_info(category: &str, fixture_name: &str) -> Option<(u16, Option<String>, Vec<String>, Vec<String>)> {
    let spec_path = PathBuf::from("tests/fixtures")
        .join(category)
        .join(fixture_name)
        .join("universalbuild.json");

    if !spec_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&spec_path).ok()?;
    let ub: Vec<UniversalBuild> = serde_json::from_str(&content)
        .or_else(|_| {
            serde_json::from_str::<UniversalBuild>(&content).map(|single| vec![single])
        })
        .ok()?;

    let first = ub.first()?;
    let port = first.runtime.ports.first().copied()?;
    let health = first.runtime.health.as_ref().map(|h| h.endpoint.clone());
    let command = first.runtime.command.clone();
    let env: Vec<String> = first.runtime.env.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();

    Some((port, health, command, env))
}

/// Helper to run container integration test for a single fixture
async fn run_container_integration_test(
    category: &str,
    fixture_name: &str,
) -> Result<(), String> {
    // Setup test APKINDEX cache
    setup_test_apkindex_cache();

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
    let harness = ContainerTestHarness::new()
        .map_err(|e| format!("Failed to create harness: {}", e))?;

    let image_name = format!(
        "localhost/aipack-test-{}-{}:latest",
        category.replace("/", "-"),
        fixture_name
    );

    let image = harness
        .build_image(&spec_path, &fixture_path, &image_name)
        .await
        .map_err(|e| format!("Failed to build image: {}", e))?;

    let container_id = harness
        .start_container(&image, port, Some(cmd), if env.is_empty() { None } else { Some(env) })
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

/// Container integration test for single-language fixtures
/// Tests run in parallel using dynamic port allocation
#[parameterized(
    rust_cargo = { "rust-cargo" },
    go_mod = { "go-mod" },
    python_pip = { "python-pip" },
    python_poetry = { "python-poetry" },
    node_npm = { "node-npm" },
    ruby_bundler = { "ruby-bundler" },
    java_maven = { "java-maven" },
    java_gradle = { "java-gradle" },
    dotnet_csproj = { "dotnet-csproj" },
    php_symfony = { "php-symfony" },
)]
fn test_container_integration_single_language(fixture_name: &str) {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    runtime.block_on(async {
        run_container_integration_test("single-language", fixture_name)
            .await
            .expect("Container integration test failed");
    });
}
