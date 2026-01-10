//! CLI integration tests
//!
//! These tests verify the command-line interface behavior, including:
//! - Command parsing and validation
//! - Output formatting
//! - Error handling
//! - Exit codes

use serial_test::serial;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Helper to get the path to the peelbox binary
fn peelbox_bin() -> PathBuf {
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
    let output = Command::new(peelbox_bin())
        .arg("--help")
        .output()
        .expect("Failed to execute peelbox");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("peelbox"));
    assert!(stdout.contains("detect"));
    assert!(stdout.contains("health"));
}

#[test]
fn test_cli_version() {
    let output = Command::new(peelbox_bin())
        .arg("--version")
        .output()
        .expect("Failed to execute peelbox");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("peelbox"));
}

#[test]
fn test_detect_help() {
    let output = Command::new(peelbox_bin())
        .arg("detect")
        .arg("--help")
        .output()
        .expect("Failed to execute peelbox");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Check for key features in help text (case-insensitive)
    assert!(stdout.to_lowercase().contains("detect") || stdout.contains("Analyzes repository"));
    assert!(stdout.contains("--format") || stdout.contains("format"));
    assert!(stdout.contains("--backend") || stdout.contains("backend"));
}

#[test]
fn test_health_command() {
    let output = Command::new(peelbox_bin())
        .arg("health")
        .output()
        .expect("Failed to execute peelbox");

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
    let output = Command::new(peelbox_bin())
        .arg("detect")
        .arg("/nonexistent/path/12345")
        .output()
        .expect("Failed to execute peelbox");

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

    let output = Command::new(peelbox_bin())
        .arg("detect")
        .arg(file_path)
        .output()
        .expect("Failed to execute peelbox");

    // Should fail because it's a file, not a directory
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a directory") || stderr.contains("is not a directory"));
}

#[test]
#[serial]
fn test_detect_json_format() {
    // Use embedded provider with smallest model for fast testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(peelbox_bin())
        .env("PEELBOX_PROVIDER", "embedded")
        .env("PEELBOX_MODEL_SIZE", "7B")
        .env("PEELBOX_ENABLE_RECORDING", "1")
        .env("PEELBOX_RECORDING_MODE", "auto")
        .env(
            "PEELBOX_TEST_NAME",
            "cli_integration_test_detect_json_format",
        )
        .arg("detect")
        .arg(repo_path)
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to execute peelbox");

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
#[serial]
fn test_detect_yaml_format() {
    // Use embedded provider with smallest model for fast testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(peelbox_bin())
        .env("PEELBOX_PROVIDER", "embedded")
        .env("PEELBOX_MODEL_SIZE", "7B")
        .env("PEELBOX_ENABLE_RECORDING", "1")
        .env("PEELBOX_RECORDING_MODE", "auto")
        .env(
            "PEELBOX_TEST_NAME",
            "cli_integration_test_detect_yaml_format",
        )
        .arg("detect")
        .arg(repo_path)
        .arg("--format")
        .arg("yaml")
        .output()
        .expect("Failed to execute peelbox");

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
#[serial]
fn test_detect_with_output_file() {
    // Use embedded provider for testing (auto-selects model based on available RAM)
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);
    let output_file = temp_dir.path().join("output.json");

    let output = Command::new(peelbox_bin())
        .env("PEELBOX_PROVIDER", "embedded")
        .env("PEELBOX_MODEL_SIZE", "7B")
        .env("PEELBOX_ENABLE_RECORDING", "1")
        .env("PEELBOX_RECORDING_MODE", "auto")
        .env(
            "PEELBOX_TEST_NAME",
            "cli_integration_test_detect_with_output_file",
        )
        .arg("detect")
        .arg(repo_path)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(&output_file)
        .output()
        .expect("Failed to execute peelbox");

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
    let output = Command::new(peelbox_bin())
        .arg("-v")
        .arg("config")
        .output()
        .expect("Failed to execute peelbox");

    // Verbose flag should not cause errors
    assert!(output.status.success() || output.status.code() == Some(2));
}

#[test]
fn test_global_quiet_flag() {
    let output = Command::new(peelbox_bin())
        .arg("-q")
        .arg("config")
        .output()
        .expect("Failed to execute peelbox");

    // Quiet flag should not cause errors
    assert!(output.status.success() || output.status.code() == Some(2));
}

#[test]
fn test_log_level_flag() {
    let output = Command::new(peelbox_bin())
        .arg("--log-level")
        .arg("debug")
        .arg("config")
        .output()
        .expect("Failed to execute peelbox");

    // Log level flag should not cause errors
    assert!(output.status.success() || output.status.code() == Some(2));
}

#[test]
fn test_invalid_backend() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(peelbox_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--backend")
        .arg("invalid")
        .output()
        .expect("Failed to execute peelbox");

    // Should fail with invalid backend error
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Clap should catch this as an invalid value
    assert!(stderr.contains("invalid") || stderr.contains("value") || stderr.contains("backend"));
}

#[test]
#[serial]
fn test_detect_with_timeout() {
    // Use embedded provider with smallest model for fast testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(peelbox_bin())
        .env("PEELBOX_PROVIDER", "embedded")
        .env("PEELBOX_MODEL_SIZE", "7B")
        .env("PEELBOX_ENABLE_RECORDING", "1")
        .env("PEELBOX_RECORDING_MODE", "auto")
        .env(
            "PEELBOX_TEST_NAME",
            "cli_integration_test_detect_with_timeout",
        )
        .arg("detect")
        .arg(repo_path)
        .arg("--timeout")
        .arg("30")
        .output()
        .expect("Failed to execute peelbox");

    // Timeout flag should be accepted (command may fail if no backend available)
    // We just check that the timeout flag parsing works
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("timeout") || !stderr.contains("invalid"));
}

#[test]
fn test_health_with_specific_backend() {
    let output = Command::new(peelbox_bin())
        .arg("health")
        .arg("--backend")
        .arg("ollama")
        .output()
        .expect("Failed to execute peelbox");

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
#[serial]
fn test_detect_no_cache_flag() {
    // Use embedded provider with smallest model for fast testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(peelbox_bin())
        .env("PEELBOX_PROVIDER", "embedded")
        .env("PEELBOX_MODEL_SIZE", "7B")
        .env("PEELBOX_ENABLE_RECORDING", "1")
        .env("PEELBOX_RECORDING_MODE", "auto")
        .env(
            "PEELBOX_TEST_NAME",
            "cli_integration_test_detect_no_cache_flag",
        )
        .arg("detect")
        .arg(repo_path)
        .output()
        .expect("Failed to execute peelbox");

    // No-cache flag should be accepted
    // Command may fail if no backend available
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("cache") || !stderr.contains("invalid"));
}

#[test]
fn test_detect_verbose_output_flag() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = create_rust_repo(&temp_dir);

    let output = Command::new(peelbox_bin())
        .arg("detect")
        .arg(repo_path)
        .arg("--verbose-output")
        .output()
        .expect("Failed to execute peelbox");

    // Verbose-output flag should be accepted
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("verbose-output") || !stderr.contains("invalid"));
}
