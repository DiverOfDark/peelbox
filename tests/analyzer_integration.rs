//! Integration tests for the repository analyzer
//!
//! These tests verify the complete workflow of analyzing different types
//! of repositories with various build systems and configurations.

use aipack::detection::analyzer::{AnalysisError, AnalyzerConfig, RepositoryAnalyzer};
use std::fs;
use tempfile::TempDir;

/// Helper to create a Rust project fixture
fn create_rust_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create Cargo.toml
    fs::write(
        repo_path.join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = "1.0"
"#,
    )
    .unwrap();

    // Create Cargo.lock
    fs::write(repo_path.join("Cargo.lock"), "# Cargo.lock\n").unwrap();

    // Create README
    fs::write(
        repo_path.join("README.md"),
        r#"# Test Project

A sample Rust project for testing.

## Building

```bash
cargo build
```
"#,
    )
    .unwrap();

    // Create src directory
    fs::create_dir(repo_path.join("src")).unwrap();
    fs::write(
        repo_path.join("src/main.rs"),
        r#"fn main() {
    println!("Hello, world!");
}
"#,
    )
    .unwrap();

    // Create tests directory
    fs::create_dir(repo_path.join("tests")).unwrap();
    fs::write(
        repo_path.join("tests/integration.rs"),
        r#"#[test]
fn test_example() {
    assert_eq!(1 + 1, 2);
}
"#,
    )
    .unwrap();

    temp_dir
}

/// Helper to create a Node.js project fixture
fn create_nodejs_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create package.json
    fs::write(
        repo_path.join("package.json"),
        r#"{
  "name": "test-app",
  "version": "1.0.0",
  "scripts": {
    "build": "webpack",
    "test": "jest"
  }
}
"#,
    )
    .unwrap();

    // Create package-lock.json
    fs::write(repo_path.join("package-lock.json"), "{}").unwrap();

    // Create README
    fs::write(
        repo_path.join("README.md"),
        "# Test Node.js App\n\nA sample Node.js project.",
    )
    .unwrap();

    // Create src directory
    fs::create_dir(repo_path.join("src")).unwrap();
    fs::write(
        repo_path.join("src/index.js"),
        "console.log('Hello, world!');",
    )
    .unwrap();

    temp_dir
}

/// Helper to create a Go project fixture
fn create_go_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create go.mod
    fs::write(
        repo_path.join("go.mod"),
        r#"module example.com/test

go 1.21
"#,
    )
    .unwrap();

    // Create go.sum
    fs::write(repo_path.join("go.sum"), "").unwrap();

    // Create main.go
    fs::write(
        repo_path.join("main.go"),
        r#"package main

import "fmt"

func main() {
    fmt.Println("Hello, world!")
}
"#,
    )
    .unwrap();

    temp_dir
}

/// Helper to create a Python project fixture
fn create_python_project() -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create pyproject.toml
    fs::write(
        repo_path.join("pyproject.toml"),
        r#"[tool.poetry]
name = "test-app"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.9"
"#,
    )
    .unwrap();

    // Create setup.py
    fs::write(
        repo_path.join("setup.py"),
        r#"from setuptools import setup

setup(
    name="test-app",
    version="0.1.0",
)
"#,
    )
    .unwrap();

    // Create requirements.txt
    fs::write(repo_path.join("requirements.txt"), "requests==2.28.0\n").unwrap();

    // Create main module
    fs::write(
        repo_path.join("main.py"),
        r#"def main():
    print("Hello, world!")

if __name__ == "__main__":
    main()
"#,
    )
    .unwrap();

    temp_dir
}

#[tokio::test]
async fn test_analyze_rust_project() {
    let project = create_rust_project();
    let analyzer = RepositoryAnalyzer::new(project.path().to_path_buf());

    let context = analyzer.analyze().await.unwrap();

    // Verify Cargo.toml was detected
    assert!(context.has_file("Cargo.toml"));
    assert!(context.key_files.contains_key("Cargo.toml"));

    // Verify content
    let cargo_toml = &context.key_files["Cargo.toml"];
    assert!(cargo_toml.contains("test-project"));
    assert!(cargo_toml.contains("tokio"));

    // Verify README
    assert!(context.readme_content.is_some());
    let readme = context.readme_content.unwrap();
    assert!(readme.contains("Test Project"));
    assert!(readme.contains("cargo build"));

    // Verify file tree
    assert!(context.file_tree.contains("Cargo.toml"));
    assert!(context.file_tree.contains("src/"));

    // Verify detected files list
    assert!(context.detected_files.contains(&"Cargo.toml".to_string()));
    assert!(context.detected_files.contains(&"Cargo.lock".to_string()));
}

#[tokio::test]
async fn test_analyze_nodejs_project() {
    let project = create_nodejs_project();
    let analyzer = RepositoryAnalyzer::new(project.path().to_path_buf());

    let context = analyzer.analyze().await.unwrap();

    // Verify package.json was detected
    assert!(context.has_file("package.json"));
    assert!(context.key_files.contains_key("package.json"));

    // Verify content
    let package_json = &context.key_files["package.json"];
    assert!(package_json.contains("test-app"));
    assert!(package_json.contains("webpack"));

    // Verify both package files detected
    assert!(context.detected_files.contains(&"package.json".to_string()));
    assert!(context
        .detected_files
        .contains(&"package-lock.json".to_string()));

    // Verify README
    assert!(context.readme_content.is_some());
}

#[tokio::test]
async fn test_analyze_go_project() {
    let project = create_go_project();
    let analyzer = RepositoryAnalyzer::new(project.path().to_path_buf());

    let context = analyzer.analyze().await.unwrap();

    // Verify go.mod was detected
    assert!(context.has_file("go.mod"));
    assert!(context.key_files.contains_key("go.mod"));

    // Verify content
    let go_mod = &context.key_files["go.mod"];
    assert!(go_mod.contains("example.com/test"));
    assert!(go_mod.contains("go 1.21"));

    // Verify both go files detected
    assert!(context.detected_files.contains(&"go.mod".to_string()));
    assert!(context.detected_files.contains(&"go.sum".to_string()));
}

#[tokio::test]
async fn test_analyze_python_project() {
    let project = create_python_project();
    let analyzer = RepositoryAnalyzer::new(project.path().to_path_buf());

    let context = analyzer.analyze().await.unwrap();

    // Verify multiple Python config files detected
    assert!(context.has_file("pyproject.toml"));
    assert!(context.has_file("setup.py"));
    assert!(context.has_file("requirements.txt"));

    // Verify all detected
    assert!(context
        .detected_files
        .contains(&"pyproject.toml".to_string()));
    assert!(context.detected_files.contains(&"setup.py".to_string()));
    assert!(context
        .detected_files
        .contains(&"requirements.txt".to_string()));

    // Verify content
    assert!(context.key_files["pyproject.toml"].contains("poetry"));
    assert!(context.key_files["setup.py"].contains("setuptools"));
    assert!(context.key_files["requirements.txt"].contains("requests"));
}

#[tokio::test]
async fn test_analyze_with_custom_config() {
    let project = create_rust_project();

    let config = AnalyzerConfig {
        max_depth: 1,
        file_tree_limit: 50,
        ..Default::default()
    };

    let analyzer = RepositoryAnalyzer::with_config(project.path().to_path_buf(), config);
    let context = analyzer.analyze().await.unwrap();

    // Should still detect root-level files
    assert!(context.has_file("Cargo.toml"));
    assert!(context.readme_content.is_some());
}

#[tokio::test]
async fn test_analyze_respects_ignore_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create files including ones that should be ignored
    fs::write(repo_path.join("Cargo.toml"), "[package]").unwrap();
    fs::create_dir(repo_path.join("target")).unwrap();
    fs::write(repo_path.join("target/debug.txt"), "build output").unwrap();
    fs::create_dir(repo_path.join("node_modules")).unwrap();
    fs::write(repo_path.join("node_modules/lib.js"), "library").unwrap();

    let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
    let context = analyzer.analyze().await.unwrap();

    // Should include Cargo.toml
    assert!(context.file_tree.contains("Cargo.toml"));

    // Should not include ignored directories' contents
    // (the directory name might appear but not its contents)
    assert!(!context.file_tree.contains("debug.txt"));
    assert!(!context.file_tree.contains("lib.js"));
}

#[tokio::test]
async fn test_analyze_empty_repository() {
    let temp_dir = TempDir::new().unwrap();
    let analyzer = RepositoryAnalyzer::new(temp_dir.path().to_path_buf());

    let context = analyzer.analyze().await.unwrap();

    // Should complete without errors
    assert_eq!(context.key_file_count(), 0);
    assert!(context.readme_content.is_none());
    assert!(context.detected_files.is_empty());
}

#[tokio::test]
async fn test_analyze_nonexistent_path() {
    let analyzer = RepositoryAnalyzer::new("/nonexistent/path/to/repo".into());

    let result = analyzer.analyze().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AnalysisError::PathNotFound(_) => {
            // Expected error
        }
        other => panic!("Expected PathNotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn test_analyze_file_instead_of_directory() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("file.txt");
    fs::write(&file_path, "content").unwrap();

    let analyzer = RepositoryAnalyzer::new(file_path);
    let result = analyzer.analyze().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        AnalysisError::NotADirectory(_) => {
            // Expected error
        }
        other => panic!("Expected NotADirectory, got {:?}", other),
    }
}

#[tokio::test]
async fn test_analyze_multi_language_project() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create a project with multiple build systems
    fs::write(repo_path.join("Cargo.toml"), "[package]").unwrap();
    fs::write(repo_path.join("package.json"), "{}").unwrap();
    fs::write(repo_path.join("go.mod"), "module test").unwrap();
    fs::write(repo_path.join("Makefile"), "all:").unwrap();
    fs::write(repo_path.join("Dockerfile"), "FROM alpine").unwrap();

    let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
    let context = analyzer.analyze().await.unwrap();

    // Should detect all build system files
    assert!(context.has_file("Cargo.toml"));
    assert!(context.has_file("package.json"));
    assert!(context.has_file("go.mod"));
    assert!(context.has_file("Makefile"));
    assert!(context.has_file("Dockerfile"));

    // All should be in detected files list
    assert_eq!(context.key_file_count(), 5);
}

#[tokio::test]
async fn test_analyze_nested_structure() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create nested structure
    fs::write(repo_path.join("Cargo.toml"), "[workspace]").unwrap();
    fs::create_dir_all(repo_path.join("crates/lib1")).unwrap();
    fs::write(
        repo_path.join("crates/lib1/Cargo.toml"),
        "[package]\nname=\"lib1\"",
    )
    .unwrap();
    fs::create_dir_all(repo_path.join("crates/lib2")).unwrap();
    fs::write(
        repo_path.join("crates/lib2/Cargo.toml"),
        "[package]\nname=\"lib2\"",
    )
    .unwrap();

    let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
    let context = analyzer.analyze().await.unwrap();

    // Should detect workspace Cargo.toml
    assert!(context.has_file("Cargo.toml"));

    // Should detect nested Cargo.toml files
    assert!(context.has_file("crates/lib1/Cargo.toml"));
    assert!(context.has_file("crates/lib2/Cargo.toml"));

    // File tree should show structure
    assert!(context.file_tree.contains("crates/"));
}

#[tokio::test]
async fn test_readme_variants() {
    // Test different README naming conventions
    let readme_names = vec![
        "README.md",
        "README.MD",
        "readme.md",
        "README.txt",
        "README",
    ];

    for readme_name in readme_names {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        fs::write(
            repo_path.join(readme_name),
            format!("# Test README ({})", readme_name),
        )
        .unwrap();

        let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
        let context = analyzer.analyze().await.unwrap();

        assert!(
            context.readme_content.is_some(),
            "Failed to detect {}",
            readme_name
        );
        assert!(context
            .readme_content
            .unwrap()
            .contains(&format!("({})", readme_name)));
    }
}
