//! Integration tests for bootstrap analysis

use aipack::bootstrap::BootstrapScanner;
use std::fs;
use tempfile::TempDir;

fn create_rust_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let base = dir.path();

    fs::write(
        base.join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"

[dependencies]
tokio = "1.0"
"#,
    )
    .unwrap();

    fs::write(base.join("Cargo.lock"), "# Cargo lock file").unwrap();

    fs::create_dir(base.join("src")).unwrap();
    fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();

    fs::write(base.join("README.md"), "# Test Project").unwrap();

    dir
}

fn create_node_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    let base = dir.path();

    fs::write(
        base.join("package.json"),
        r#"{
  "name": "test-project",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc",
    "test": "jest"
  }
}"#,
    )
    .unwrap();

    fs::write(base.join("package-lock.json"), "{}").unwrap();

    fs::create_dir(base.join("src")).unwrap();
    fs::write(base.join("src/index.ts"), "console.log('test');").unwrap();

    dir
}

fn create_monorepo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let base = dir.path();

    fs::write(
        base.join("pnpm-workspace.yaml"),
        "packages:\n  - 'packages/*'\n",
    )
    .unwrap();

    fs::write(
        base.join("package.json"),
        r#"{"name": "monorepo", "version": "1.0.0"}"#,
    )
    .unwrap();

    fs::create_dir(base.join("packages")).unwrap();
    fs::create_dir(base.join("packages/app")).unwrap();
    fs::write(
        base.join("packages/app/package.json"),
        r#"{"name": "@monorepo/app"}"#,
    )
    .unwrap();

    fs::create_dir(base.join("packages/lib")).unwrap();
    fs::write(
        base.join("packages/lib/package.json"),
        r#"{"name": "@monorepo/lib"}"#,
    )
    .unwrap();

    dir
}

#[test]
fn test_scan_rust_project() {
    let project = create_rust_project();
    let scanner = BootstrapScanner::new(project.path().to_path_buf()).unwrap();

    let context = scanner.scan().unwrap();

    assert!(!context.detections.is_empty());

    let has_rust = context.detections.iter().any(|d| d.language == "Rust");
    assert!(has_rust, "Should detect Rust");

    let rust_detection = context
        .detections
        .iter()
        .find(|d| d.language == "Rust")
        .unwrap();
    assert_eq!(rust_detection.build_system, "cargo");
}

#[test]
fn test_scan_node_project() {
    let project = create_node_project();
    let scanner = BootstrapScanner::new(project.path().to_path_buf()).unwrap();

    let context = scanner.scan().unwrap();

    assert!(!context.detections.is_empty());

    let has_js = context
        .detections
        .iter()
        .any(|d| d.language == "JavaScript");
    assert!(has_js, "Should detect JavaScript");
}

#[test]
fn test_scan_excludes_ignored_directories() {
    let dir = TempDir::new().unwrap();
    let base = dir.path();

    fs::write(
        base.join("package.json"),
        r#"{"name": "test", "version": "1.0.0"}"#,
    )
    .unwrap();

    fs::create_dir(base.join("node_modules")).unwrap();
    fs::write(base.join("node_modules/package.json"), "{}").unwrap();

    fs::create_dir(base.join("target")).unwrap();
    fs::write(base.join("target/Cargo.toml"), "{}").unwrap();

    let scanner = BootstrapScanner::new(base.to_path_buf()).unwrap();
    let context = scanner.scan().unwrap();

    assert_eq!(
        context.detections.len(),
        1,
        "Should only find root package.json, not ignored ones"
    );
    assert_eq!(context.detections[0].language, "JavaScript");
}

#[test]
fn test_context_generation_rust() {
    let project = create_rust_project();
    let scanner = BootstrapScanner::new(project.path().to_path_buf()).unwrap();

    let context = scanner.scan().unwrap();

    assert_eq!(context.summary.primary_language, Some("Rust".to_string()));
    assert_eq!(
        context.summary.primary_build_system,
        Some("cargo".to_string())
    );
    assert!(!context.summary.is_monorepo);
}

#[test]
fn test_context_generation_node() {
    let project = create_node_project();
    let scanner = BootstrapScanner::new(project.path().to_path_buf()).unwrap();

    let context = scanner.scan().unwrap();

    assert_eq!(
        context.summary.primary_language,
        Some("JavaScript".to_string())
    );
    assert_eq!(
        context.summary.primary_build_system,
        Some("npm".to_string())
    );
}

#[test]
fn test_context_generation_monorepo() {
    let project = create_monorepo();
    let scanner = BootstrapScanner::new(project.path().to_path_buf()).unwrap();

    let context = scanner.scan().unwrap();

    assert!(
        context.summary.is_monorepo,
        "Should detect monorepo structure"
    );
    assert!(
        context.detections.len() >= 3,
        "Should find multiple package.json files"
    );
}

#[test]
fn test_context_prompt_string() {
    let project = create_rust_project();
    let scanner = BootstrapScanner::new(project.path().to_path_buf()).unwrap();

    let context = scanner.scan().unwrap();

    let prompt = context.format_for_prompt();

    assert!(prompt.contains("Pre-scanned Repository"));
    assert!(prompt.contains("Cargo.toml"));
    assert!(prompt.contains("cargo"));
    assert!(prompt.contains("Rust"));
}

#[test]
fn test_workspace_info() {
    let project = create_monorepo();
    let scanner = BootstrapScanner::new(project.path().to_path_buf()).unwrap();

    let context = scanner.scan().unwrap();

    assert!(
        !context.workspace.root_manifests.is_empty(),
        "Should have root manifests"
    );
    assert!(
        context.workspace.max_depth > 0,
        "Should have nested structure"
    );
}

#[test]
fn test_scanner_performance() {
    let project = create_monorepo();
    let scanner = BootstrapScanner::new(project.path().to_path_buf()).unwrap();

    let start = std::time::Instant::now();
    let context = scanner.scan().unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 500,
        "Scan should complete in less than 500ms, took {}ms",
        elapsed.as_millis()
    );
    assert!(!context.detections.is_empty(), "Should find detections");
}
