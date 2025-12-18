//! End-to-end tests using fixtures and binary execution
//!
//! These tests verify the complete detection pipeline by spawning the aipack binary:
//! - Bootstrap scanning
//! - LLM conversation with tool calling
//! - Validation of final output
//!
//! Tests use RecordingMode::Auto to replay cached LLM responses for deterministic testing.

use aipack::output::schema::UniversalBuild;
use serial_test::serial;
use std::env;
use std::path::PathBuf;
use std::process::Command;

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
fn load_expected(fixture_name: &str) -> Option<Vec<UniversalBuild>> {
    let expected_path =
        PathBuf::from("tests/fixtures/expected").join(format!("{}.json", fixture_name));

    if !expected_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("Failed to read expected JSON: {}", expected_path.display()));

    // Try parsing as array of UniversalBuild first (for monorepos)
    if let Ok(multi) = serde_json::from_str::<Vec<UniversalBuild>>(&content) {
        return Some(multi);
    }

    // Try parsing as single UniversalBuild
    if let Ok(single) = serde_json::from_str::<UniversalBuild>(&content) {
        return Some(vec![single]);
    }

    panic!(
        "Failed to parse expected JSON as UniversalBuild or Vec<UniversalBuild>: {}",
        expected_path.display()
    )
}

/// Helper to run detection on a fixture and parse results
fn run_detection(fixture: PathBuf, test_name: &str) -> Result<Vec<UniversalBuild>, String> {
    // Create .git directory in fixture to prevent WalkBuilder from looking up the tree
    let git_dir = fixture.join(".git");
    if !git_dir.exists() {
        std::fs::create_dir_all(&git_dir).ok();
    }

    let output = Command::new(aipack_bin())
        .env("AIPACK_PROVIDER", "embedded")
        .env("AIPACK_MODEL_SIZE", "7B")
        .env("AIPACK_ENABLE_RECORDING", "1")
        .env("AIPACK_RECORDING_MODE", "auto")
        .env("AIPACK_TEST_NAME", test_name)
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
fn assert_detection(results: &[UniversalBuild], expected_build_system: &str, fixture_name: &str) {
    assert!(!results.is_empty(), "Results should not be empty");

    // Basic assertions on first result
    assert_eq!(
        results[0].metadata.build_system, expected_build_system,
        "Expected build system '{}', got '{}'",
        expected_build_system, results[0].metadata.build_system
    );

    assert!(
        !results[0].build.commands.is_empty(),
        "Build commands should not be empty"
    );

    assert!(
        results[0].metadata.confidence >= 0.5,
        "Confidence should be at least 0.5, got {}",
        results[0].metadata.confidence
    );

    // Validate against expected JSON if it exists
    if let Some(expected) = load_expected(fixture_name) {
        assert_eq!(
            results.len(),
            expected.len(),
            "Number of detected projects mismatch for {}",
            fixture_name
        );

        for (i, (detected, expected_build)) in results.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                detected.metadata.language, expected_build.metadata.language,
                "Language mismatch for project {}: expected '{}', got '{}'",
                i, expected_build.metadata.language, detected.metadata.language
            );
            assert_eq!(
                detected.metadata.build_system, expected_build.metadata.build_system,
                "Build system mismatch for project {}: expected '{}', got '{}'",
                i, expected_build.metadata.build_system, detected.metadata.build_system
            );
            assert_eq!(
                detected.build.base, expected_build.build.base,
                "Build base image mismatch for project {}: expected '{}', got '{}'",
                i, expected_build.build.base, detected.build.base
            );
            assert_eq!(
                detected.runtime.base, expected_build.runtime.base,
                "Runtime base image mismatch for project {}: expected '{}', got '{}'",
                i, expected_build.runtime.base, detected.runtime.base
            );
        }
    }
}

//
// Single-language tests
//

#[test]
#[serial]
fn test_rust_cargo_detection() {
    let fixture = fixture_path("single-language", "rust-cargo");
    let results =
        run_detection(fixture, "e2e_test_rust_cargo_detection").expect("Detection failed");

    assert_detection(&results, "Cargo", "rust-cargo");
    assert!(
        results[0]
            .build
            .commands
            .iter()
            .any(|cmd| cmd.contains("cargo build")),
        "Should contain cargo build command"
    );
}

#[test]
#[serial]
fn test_node_npm_detection() {
    let fixture = fixture_path("single-language", "node-npm");
    let results = run_detection(fixture, "e2e_test_node_npm_detection").expect("Detection failed");

    assert_detection(&results, "npm", "node-npm");
    assert!(
        results[0]
            .build
            .commands
            .iter()
            .any(|cmd| cmd.contains("npm")),
        "Should contain npm command"
    );
}

#[test]
#[serial]
fn test_python_pip_detection() {
    let fixture = fixture_path("single-language", "python-pip");
    let results =
        run_detection(fixture, "e2e_test_python_pip_detection").expect("Detection failed");

    assert_detection(&results, "pip", "python-pip");
}

#[test]
#[serial]
fn test_java_maven_detection() {
    let fixture = fixture_path("single-language", "java-maven");
    let results =
        run_detection(fixture, "e2e_test_java_maven_detection").expect("Detection failed");

    assert_detection(&results, "Maven", "java-maven");
}

#[test]
#[serial]
fn test_node_yarn_detection() {
    let fixture = fixture_path("single-language", "node-yarn");
    let results = run_detection(fixture, "e2e_test_node_yarn_detection").expect("Detection failed");

    assert_detection(&results, "npm", "node-yarn");
}

#[test]
#[serial]
fn test_node_pnpm_detection() {
    let fixture = fixture_path("single-language", "node-pnpm");
    let results = run_detection(fixture, "e2e_test_node_pnpm_detection").expect("Detection failed");

    assert_detection(&results, "npm", "node-pnpm");
}

#[test]
#[serial]
fn test_python_poetry_detection() {
    let fixture = fixture_path("single-language", "python-poetry");
    let results =
        run_detection(fixture, "e2e_test_python_poetry_detection").expect("Detection failed");

    assert_detection(&results, "Poetry", "python-poetry");
}

#[test]
#[serial]
fn test_java_gradle_detection() {
    let fixture = fixture_path("single-language", "java-gradle");
    let results =
        run_detection(fixture, "e2e_test_java_gradle_detection").expect("Detection failed");

    assert_detection(&results, "Gradle", "java-gradle");
}

#[test]
#[serial]
fn test_kotlin_gradle_detection() {
    let fixture = fixture_path("single-language", "kotlin-gradle");
    let results =
        run_detection(fixture, "e2e_test_kotlin_gradle_detection").expect("Detection failed");

    assert_detection(&results, "Gradle", "kotlin-gradle");
}

#[test]
#[serial]
fn test_dotnet_csproj_detection() {
    let fixture = fixture_path("single-language", "dotnet-csproj");
    let results =
        run_detection(fixture, "e2e_test_dotnet_csproj_detection").expect("Detection failed");

    assert_detection(&results, ".NET", "dotnet-csproj");
}

#[test]
#[serial]
fn test_go_mod_detection() {
    let fixture = fixture_path("single-language", "go-mod");
    let results = run_detection(fixture, "e2e_test_go_mod_detection").expect("Detection failed");

    assert_detection(&results, "go mod", "go-mod");
}

#[test]
#[serial]
fn test_ruby_bundler_detection() {
    let fixture = fixture_path("single-language", "ruby-bundler");
    let results =
        run_detection(fixture, "e2e_test_ruby_bundler_detection").expect("Detection failed");

    assert_detection(&results, "Bundler", "ruby-bundler");
}

#[test]
#[serial]
fn test_php_composer_detection() {
    let fixture = fixture_path("single-language", "php-composer");
    let results =
        run_detection(fixture, "e2e_test_php_composer_detection").expect("Detection failed");

    assert_detection(&results, "Composer", "php-composer");
}

#[test]
#[serial]
fn test_cpp_cmake_detection() {
    let fixture = fixture_path("single-language", "cpp-cmake");
    let results = run_detection(fixture, "e2e_test_cpp_cmake_detection").expect("Detection failed");

    assert_detection(&results, "CMake", "cpp-cmake");
}

#[test]
#[serial]
fn test_elixir_mix_detection() {
    let fixture = fixture_path("single-language", "elixir-mix");
    let results =
        run_detection(fixture, "e2e_test_elixir_mix_detection").expect("Detection failed");

    assert_detection(&results, "Mix", "elixir-mix");
}

//
// Special case tests
//

#[test]
#[serial]
fn test_empty_repo_detection() {
    let fixture = fixture_path("edge-cases", "empty-repo");
    let result = run_detection(fixture, "e2e_test_empty_repo_detection");

    // Empty repo should fail detection or return error
    assert!(
        result.is_err() || result.unwrap().is_empty(),
        "Empty repo should fail detection"
    );
}

#[test]
#[serial]
fn test_no_manifest_detection() {
    let fixture = fixture_path("edge-cases", "no-manifest");
    let result = run_detection(fixture, "e2e_test_no_manifest_detection");

    // No manifest should fail or return low confidence
    assert!(
        result.is_err() || result.unwrap().iter().all(|r| r.metadata.confidence < 0.5),
        "No manifest should result in low confidence or failure"
    );
}

//
// Monorepo tests
//

#[test]
#[serial]
fn test_rust_workspace_detection() {
    let fixture = fixture_path("monorepo", "cargo-workspace");
    let results =
        run_detection(fixture, "e2e_test_rust_workspace_detection").expect("Detection failed");

    assert_detection(&results, "Cargo", "cargo-workspace");
}

#[test]
#[serial]
fn test_npm_workspaces_detection() {
    let fixture = fixture_path("monorepo", "npm-workspaces");
    let results =
        run_detection(fixture, "e2e_test_npm_workspaces_detection").expect("Detection failed");

    assert_detection(&results, "npm", "npm-workspaces");
}

#[test]
#[serial]
fn test_cargo_workspace_detection() {
    let fixture = fixture_path("monorepo", "cargo-workspace");
    let results =
        run_detection(fixture, "e2e_test_cargo_workspace_detection").expect("Detection failed");

    // Workspace should detect cargo as build system
    assert!(!results.is_empty(), "Should detect workspace");
    assert_eq!(results[0].metadata.build_system, "Cargo");
}

#[test]
#[serial]
fn test_turborepo_detection() {
    let fixture = fixture_path("monorepo", "turborepo");
    let results = run_detection(fixture, "e2e_test_turborepo_detection").expect("Detection failed");

    assert_detection(&results, "npm", "turborepo");
}

#[test]
#[serial]
fn test_gradle_multiproject_detection() {
    let fixture = fixture_path("monorepo", "gradle-multiproject");
    let results =
        run_detection(fixture, "e2e_test_gradle_multiproject_detection").expect("Detection failed");

    assert_detection(&results, "Gradle", "gradle-multiproject");
}

#[test]
#[serial]
fn test_maven_multimodule_detection() {
    let fixture = fixture_path("monorepo", "maven-multimodule");
    let results =
        run_detection(fixture, "e2e_test_maven_multimodule_detection").expect("Detection failed");

    assert_detection(&results, "Maven", "maven-multimodule");
}

#[test]
#[serial]
fn test_polyglot_detection() {
    let fixture = fixture_path("monorepo", "polyglot");
    let results = run_detection(fixture, "e2e_test_polyglot_detection").expect("Detection failed");

    // Polyglot should detect multiple languages or pick primary one
    assert!(!results.is_empty(), "Should detect at least one language");
}
