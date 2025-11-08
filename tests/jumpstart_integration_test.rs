//! Integration tests for jumpstart analysis

use aipack::detection::jumpstart::{JumpstartContext, JumpstartScanner};
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
    let scanner = JumpstartScanner::new(project.path().to_path_buf()).unwrap();

    let manifests = scanner.scan().unwrap();

    assert!(manifests.len() >= 1);

    let has_cargo_toml = manifests.iter().any(|m| m.name == "Cargo.toml");
    assert!(has_cargo_toml, "Should find Cargo.toml");

    let cargo_toml = manifests.iter().find(|m| m.name == "Cargo.toml").unwrap();
    assert_eq!(cargo_toml.depth, 0, "Cargo.toml should be at root level");
}

#[test]
fn test_scan_node_project() {
    let project = create_node_project();
    let scanner = JumpstartScanner::new(project.path().to_path_buf()).unwrap();

    let manifests = scanner.scan().unwrap();

    assert!(manifests.len() >= 1);

    let has_package_json = manifests.iter().any(|m| m.name == "package.json");
    assert!(has_package_json, "Should find package.json");
}

#[test]
fn test_scan_excludes_ignored_directories() {
    let dir = TempDir::new().unwrap();
    let base = dir.path();

    fs::write(base.join("package.json"), "{}").unwrap();

    fs::create_dir(base.join("node_modules")).unwrap();
    fs::write(base.join("node_modules/package.json"), "{}").unwrap();

    fs::create_dir(base.join("target")).unwrap();
    fs::write(base.join("target/Cargo.toml"), "{}").unwrap();

    let scanner = JumpstartScanner::new(base.to_path_buf()).unwrap();
    let manifests = scanner.scan().unwrap();

    assert_eq!(
        manifests.len(),
        1,
        "Should only find root package.json, not ignored ones"
    );
    assert_eq!(manifests[0].name, "package.json");
    assert_eq!(manifests[0].depth, 0);
}

#[test]
fn test_context_generation_rust() {
    let project = create_rust_project();
    let scanner = JumpstartScanner::new(project.path().to_path_buf()).unwrap();

    let manifests = scanner.scan().unwrap();
    let context = JumpstartContext::from_manifests(manifests, 100);

    assert!(context
        .project_hints
        .detected_languages
        .contains(&"Rust".to_string()));
    assert_eq!(
        context.project_hints.likely_build_system,
        Some("Cargo".to_string())
    );
    assert!(!context.project_hints.is_monorepo);
}

#[test]
fn test_context_generation_node() {
    let project = create_node_project();
    let scanner = JumpstartScanner::new(project.path().to_path_buf()).unwrap();

    let manifests = scanner.scan().unwrap();
    let context = JumpstartContext::from_manifests(manifests, 100);

    assert!(context
        .project_hints
        .detected_languages
        .contains(&"JavaScript/TypeScript".to_string()));
    assert_eq!(
        context.project_hints.likely_build_system,
        Some("npm/yarn/pnpm".to_string())
    );
}

#[test]
fn test_context_generation_monorepo() {
    let project = create_monorepo();
    let scanner = JumpstartScanner::new(project.path().to_path_buf()).unwrap();

    let manifests = scanner.scan().unwrap();
    let manifest_count = manifests.len();
    let context = JumpstartContext::from_manifests(manifests, 100);

    assert!(
        context.project_hints.is_monorepo,
        "Should detect monorepo structure"
    );
    assert!(
        manifest_count >= 3,
        "Should find multiple package.json files"
    );
}

#[test]
fn test_context_prompt_string() {
    let project = create_rust_project();
    let scanner = JumpstartScanner::new(project.path().to_path_buf()).unwrap();

    let manifests = scanner.scan().unwrap();
    let context = JumpstartContext::from_manifests(manifests, 100);

    let prompt = context.to_prompt_string();

    assert!(prompt.contains("Pre-scanned repository"));
    assert!(prompt.contains("Cargo.toml"));
    assert!(prompt.contains("Likely build system"));
    assert!(prompt.contains("Cargo"));
    assert!(prompt.contains("Rust"));
}

#[test]
fn test_workspace_info() {
    let project = create_monorepo();
    let scanner = JumpstartScanner::new(project.path().to_path_buf()).unwrap();

    let manifests = scanner.scan().unwrap();
    let context = JumpstartContext::from_manifests(manifests, 100);

    assert!(
        context.workspace_info.root_manifests.len() >= 1,
        "Should have root manifests"
    );
    assert!(
        context.workspace_info.max_depth > 0,
        "Should have nested structure"
    );
}

#[test]
fn test_scanner_respects_depth_limit() {
    let dir = TempDir::new().unwrap();
    let base = dir.path();

    fs::write(base.join("Cargo.toml"), "[package]").unwrap();

    fs::create_dir(base.join("level1")).unwrap();
    fs::write(base.join("level1/Cargo.toml"), "[package]").unwrap();

    fs::create_dir(base.join("level1/level2")).unwrap();
    fs::write(base.join("level1/level2/Cargo.toml"), "[package]").unwrap();

    let scanner = JumpstartScanner::with_limits(base.to_path_buf(), 1, 1000).unwrap();
    let manifests = scanner.scan().unwrap();

    assert!(
        manifests.iter().all(|m| m.depth <= 1),
        "Should respect max depth limit"
    );
}

#[test]
fn test_scanner_performance() {
    let project = create_monorepo();
    let scanner = JumpstartScanner::new(project.path().to_path_buf()).unwrap();

    let start = std::time::Instant::now();
    let manifests = scanner.scan().unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 500,
        "Scan should complete in less than 500ms, took {}ms",
        elapsed.as_millis()
    );
    assert!(manifests.len() > 0, "Should find manifests");
}
