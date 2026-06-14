//! CLI integration tests
//!
//! These tests verify the command-line surface: argument parsing, help/version
//! output, and error handling. Full detection behavior is covered by the
//! fixture-driven `static_e2e` suite.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

mod support;

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
    assert!(stdout.contains("build"));
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
    assert!(stdout.to_lowercase().contains("detect") || stdout.contains("Analyzes repository"));
    assert!(stdout.contains("--format"));
    assert!(stdout.contains("--output"));
}

#[test]
fn test_build_help() {
    let output = Command::new(peelbox_bin())
        .arg("build")
        .arg("--help")
        .output()
        .expect("Failed to execute peelbox");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--spec"));
    assert!(stdout.contains("--tag"));
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
    let temp_dir =
        TempDir::new_in(support::get_test_temp_dir()).expect("Failed to create temp dir");
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
fn test_global_quiet_flag() {
    let output = Command::new(peelbox_bin())
        .arg("-q")
        .arg("detect")
        .arg("--help")
        .output()
        .expect("Failed to execute peelbox");

    assert!(output.status.success());
}

#[test]
fn test_log_level_flag() {
    let output = Command::new(peelbox_bin())
        .arg("--log-level")
        .arg("debug")
        .arg("detect")
        .arg("--help")
        .output()
        .expect("Failed to execute peelbox");

    assert!(output.status.success());
}

#[test]
fn test_unknown_command_fails() {
    let output = Command::new(peelbox_bin())
        .arg("definitely-not-a-command")
        .output()
        .expect("Failed to execute peelbox");

    assert!(!output.status.success());
}
