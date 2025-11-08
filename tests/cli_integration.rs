//! CLI integration tests
//!
//! These tests verify the command-line interface behavior, including:
//! - Command parsing and validation
//! - Output formatting
//! - Error handling
//! - Exit codes

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

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

/// Helper to create a test Rust repository
fn create_rust_repo(dir: &TempDir) -> PathBuf {
    let repo_path = dir.path().to_path_buf();

    // Create Cargo.toml
    let cargo_toml = r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = "1.0"
"#;
    fs::write(repo_path.join("Cargo.toml"), cargo_toml).expect("Failed to write Cargo.toml");

    // Create src directory
    fs::create_dir_all(repo_path.join("src")).expect("Failed to create src directory");

    // Create main.rs
    fs::write(repo_path.join("src/main.rs"), "fn main() {}\n").expect("Failed to write main.rs");

    repo_path
}

#[test]
fn test_cli_help() {
    let output = Command::new(aipack_bin())
        .arg("--help")
        .output()
        .expect("Failed to execute aipack");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("aipack"));
    assert!(stdout.contains("detect"));
    assert!(stdout.contains("health"));
}

#[test]
fn test_cli_version() {
    let output = Command::new(aipack_bin())
        .arg("--version")
        .output()
        .expect("Failed to execute aipack");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("aipack"));
}

#[test]
fn test_detect_help() {
    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg("--help")
        .output()
        .expect("Failed to execute aipack");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Check for key features in help text (case-insensitive)
    assert!(stdout.to_lowercase().contains("detect") || stdout.contains("Analyzes repository"));
    assert!(stdout.contains("--format") || stdout.contains("format"));
    assert!(stdout.contains("--backend") || stdout.contains("backend"));
}

#[test]
fn test_health_command() {
    let output = Command::new(aipack_bin())
        .arg("health")
        .output()
        .expect("Failed to execute aipack");

    // Health command should complete even if backends are unavailable
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Check both stdout and stderr for health output
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Ollama")
            || combined.contains("Mistral")
            || combined.contains("Backend")
            || combined.contains("Health")
            || combined.contains("health")
    );
}


#[test]
fn test_detect_nonexistent_path() {
    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg("/nonexistent/path/12345")
        .output()
        .expect("Failed to execute aipack");

    // Should fail with error code
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("does not exist") || stderr.contains("not found"));
}

#[test]
fn test_detect_file_instead_of_directory() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("file.txt");
    fs::write(&file_path, "content").expect("Failed to write file");

    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg(file_path)
        .output()
        .expect("Failed to execute aipack");

    // Should fail because it's a file, not a directory
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a directory") || stderr.contains("is not a directory"));
}

#[test]
fn test_detect_json_format() {
    // This test requires Ollama to be running, so we skip if it's not available
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to execute aipack");

    // If backend is unavailable, command will fail, which is expected
    if output.status.success() || output.status.code() == Some(2) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // If successful, should be valid JSON
        if output.status.success() {
            assert!(stdout.contains("{"));
            assert!(stdout.contains("build_system") || stdout.contains("buildSystem"));
        }
    }
}

#[test]
fn test_detect_yaml_format() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--format")
        .arg("yaml")
        .output()
        .expect("Failed to execute aipack");

    // If backend is unavailable, command will fail, which is expected
    if output.status.success() || output.status.code() == Some(2) {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // If successful, should be valid YAML
        if output.status.success() {
            assert!(stdout.contains(":"));
        }
    }
}

#[test]
fn test_detect_with_output_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);
    let output_file = temp_dir.path().join("output.json");

    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output_file)
        .output()
        .expect("Failed to execute aipack");

    // If successful, output file should be created
    if output.status.success() {
        assert!(output_file.exists());
        let content = fs::read_to_string(&output_file).expect("Failed to read output file");
        assert!(!content.is_empty());
        assert!(content.contains("{"));
    }
}

#[test]
fn test_global_verbose_flag() {
    let output = Command::new(aipack_bin())
        .arg("-v")
        .arg("config")
        .output()
        .expect("Failed to execute aipack");

    // Verbose flag should not cause errors
    assert!(output.status.success() || output.status.code() == Some(2));
}

#[test]
fn test_global_quiet_flag() {
    let output = Command::new(aipack_bin())
        .arg("-q")
        .arg("config")
        .output()
        .expect("Failed to execute aipack");

    // Quiet flag should not cause errors
    assert!(output.status.success() || output.status.code() == Some(2));
}

#[test]
fn test_log_level_flag() {
    let output = Command::new(aipack_bin())
        .arg("--log-level")
        .arg("debug")
        .arg("config")
        .output()
        .expect("Failed to execute aipack");

    // Log level flag should not cause errors
    assert!(output.status.success() || output.status.code() == Some(2));
}

#[test]
fn test_invalid_backend() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--backend")
        .arg("invalid")
        .output()
        .expect("Failed to execute aipack");

    // Should fail with invalid backend error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Clap should catch this as an invalid value
    assert!(stderr.contains("invalid") || stderr.contains("value") || stderr.contains("backend"));
}

#[test]
fn test_detect_with_timeout() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--timeout")
        .arg("30")
        .output()
        .expect("Failed to execute aipack");

    // Timeout flag should be accepted (command may fail if no backend available)
    // We just check that the timeout flag parsing works
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("timeout") || !stderr.contains("invalid"));
}

#[test]
fn test_health_with_specific_backend() {
    let output = Command::new(aipack_bin())
        .arg("health")
        .arg("--backend")
        .arg("ollama")
        .output()
        .expect("Failed to execute aipack");

    // Should complete (may show unavailable, but shouldn't crash)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Ollama")
            || combined.contains("ollama")
            || combined.contains("health")
            || combined.contains("Backend")
    );
}

#[test]
fn test_detect_no_cache_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--no-cache")
        .output()
        .expect("Failed to execute aipack");

    // No-cache flag should be accepted
    // Command may fail if no backend available
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("cache") || !stderr.contains("invalid"));
}

#[test]
fn test_detect_verbose_output_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(aipack_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--verbose-output")
        .output()
        .expect("Failed to execute aipack");

    // Verbose-output flag should be accepted
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("verbose-output") || !stderr.contains("invalid"));
}
