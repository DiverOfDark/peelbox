//! End-to-end integration tests
//!
//! Tests the complete workflow from CLI to result, including:
//! - Command-line argument parsing
//! - Configuration loading
//! - Detection service initialization
//! - Repository analysis
//! - Output formatting
//! - Error handling

use aipack::cli::commands::{BackendArg, CliArgs, Commands, OutputFormatArg};
use aipack::cli::output::{OutputFormat, OutputFormatter};
use aipack::config::AipackConfig;
use aipack::detection::analyzer::RepositoryAnalyzer;
use aipack::detection::types::{DetectionResult, RepositoryContext};
use clap::Parser;
use std::env;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper function to create a test Rust project
fn create_rust_project(dir: &TempDir) -> PathBuf {
    let path = dir.path().to_path_buf();

    // Create directory structure
    fs::create_dir(path.join("src")).unwrap();
    fs::create_dir(path.join("tests")).unwrap();

    // Create Cargo.toml
    fs::write(
        path.join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"

[dev-dependencies]
tokio-test = "0.4"
"#,
    )
    .unwrap();

    // Create README
    fs::write(
        path.join("README.md"),
        r#"# Test Project

A sample Rust project for testing.

## Building

```bash
cargo build --release
```

## Testing

```bash
cargo test
```
"#,
    )
    .unwrap();

    // Create source files
    fs::write(
        path.join("src/main.rs"),
        r#"fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_main() {
        assert_eq!(2 + 2, 4);
    }
}
"#,
    )
    .unwrap();

    fs::write(
        path.join("src/lib.rs"),
        r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    )
    .unwrap();

    // Create .gitignore
    fs::write(
        path.join(".gitignore"),
        r#"target/
Cargo.lock
*.swp
"#,
    )
    .unwrap();

    path
}

/// Helper function to create a test Node.js project
fn create_nodejs_project(dir: &TempDir) -> PathBuf {
    let path = dir.path().to_path_buf();

    // Create directory structure
    fs::create_dir(path.join("src")).unwrap();
    fs::create_dir(path.join("tests")).unwrap();

    // Create package.json
    fs::write(
        path.join("package.json"),
        r#"{
  "name": "test-project",
  "version": "1.0.0",
  "description": "Test Node.js project",
  "main": "src/index.js",
  "scripts": {
    "build": "tsc",
    "test": "jest",
    "start": "node src/index.js",
    "dev": "nodemon src/index.js"
  },
  "dependencies": {
    "express": "^4.18.0"
  },
  "devDependencies": {
    "jest": "^29.0.0",
    "typescript": "^5.0.0"
  }
}
"#,
    )
    .unwrap();

    // Create tsconfig.json
    fs::write(
        path.join("tsconfig.json"),
        r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true
  }
}
"#,
    )
    .unwrap();

    // Create README
    fs::write(
        path.join("README.md"),
        "# Test Node.js Project\n\nA sample project for testing.",
    )
    .unwrap();

    // Create source files
    fs::write(
        path.join("src/index.js"),
        "console.log('Hello from Node.js!');",
    )
    .unwrap();

    path
}

#[test]
fn test_cli_parsing_detect_default() {
    let args = CliArgs::parse_from(&["aipack", "detect"]);

    match args.command {
        Commands::Detect(detect_args) => {
            assert_eq!(detect_args.format, OutputFormatArg::Human);
            assert_eq!(detect_args.backend, BackendArg::Auto);
            assert_eq!(detect_args.timeout, 60);
            assert!(detect_args.repository_path.is_none());
        }
        _ => panic!("Expected Detect command"),
    }
}

#[test]
fn test_cli_parsing_detect_with_options() {
    let args = CliArgs::parse_from(&[
        "aipack",
        "detect",
        "/tmp/repo",
        "--format",
        "json",
        "--backend",
        "ollama",
        "--timeout",
        "120",
    ]);

    match args.command {
        Commands::Detect(detect_args) => {
            assert_eq!(
                detect_args.repository_path,
                Some(PathBuf::from("/tmp/repo"))
            );
            assert_eq!(detect_args.format, OutputFormatArg::Json);
            assert_eq!(detect_args.backend, BackendArg::Ollama);
            assert_eq!(detect_args.timeout, 120);
        }
        _ => panic!("Expected Detect command"),
    }
}

#[test]
fn test_cli_parsing_health() {
    let args = CliArgs::parse_from(&["aipack", "health"]);

    match args.command {
        Commands::Health(_) => {}
        _ => panic!("Expected Health command"),
    }
}

#[test]
fn test_cli_parsing_config() {
    let args = CliArgs::parse_from(&["aipack", "config"]);

    match args.command {
        Commands::Config(_) => {}
        _ => panic!("Expected Config command"),
    }
}

#[test]
fn test_cli_parsing_invalid_command() {
    let result = CliArgs::try_parse_from(&["aipack", "invalid"]);
    assert!(result.is_err());
}

#[test]
fn test_configuration_loading_defaults() {
    // Clear relevant environment variables
    env::remove_var("AIPACK_BACKEND");
    env::remove_var("AIPACK_OLLAMA_ENDPOINT");

    let config = AipackConfig::default();

    assert_eq!(config.backend, "auto");
    assert_eq!(config.ollama_endpoint, "http://localhost:11434");
    assert_eq!(config.ollama_model, "qwen:7b");
    assert!(config.cache_enabled);
}

#[test]
fn test_configuration_validation_valid() {
    let config = AipackConfig {
        backend: "ollama".to_string(),
        ollama_endpoint: "http://localhost:11434".to_string(),
        ollama_model: "qwen:7b".to_string(),
        mistral_api_key: None,
        mistral_model: "mistral-small".to_string(),
        cache_enabled: true,
        cache_dir: Some(PathBuf::from("/tmp/cache")),
        request_timeout_secs: 30,
        max_context_size: 512_000,
        log_level: "info".to_string(),
    };

    assert!(config.validate().is_ok());
}

#[test]
fn test_configuration_validation_invalid_backend() {
    let mut config = AipackConfig::default();
    config.backend = "invalid".to_string();

    let result = config.validate();
    assert!(result.is_err());
}

#[test]
fn test_configuration_validation_invalid_timeout() {
    let mut config = AipackConfig::default();
    config.request_timeout_secs = 0;

    let result = config.validate();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_repository_analysis_rust_project() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = create_rust_project(&temp_dir);

    let analyzer = RepositoryAnalyzer::new(repo_path.clone());
    let context = analyzer.analyze().await.unwrap();

    // Verify context
    assert!(!context.file_tree.is_empty());
    assert!(context.file_tree.contains("Cargo.toml"));
    assert!(context.key_files.contains_key("Cargo.toml"));
    assert!(context.readme_content.is_some());
    assert!(!context.detected_files.is_empty());
    assert_eq!(context.repo_path, repo_path);
}

#[tokio::test]
async fn test_repository_analysis_nodejs_project() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = create_nodejs_project(&temp_dir);

    let analyzer = RepositoryAnalyzer::new(repo_path.clone());
    let context = analyzer.analyze().await.unwrap();

    // Verify context
    assert!(context.file_tree.contains("package.json"));
    assert!(context.key_files.contains_key("package.json"));
    assert!(context.key_files.contains_key("tsconfig.json"));
}

#[test]
fn test_output_format_json() {
    let result = DetectionResult {
        build_system: "cargo".to_string(),
        language: "Rust".to_string(),
        build_command: "cargo build --release".to_string(),
        test_command: "cargo test".to_string(),
        deploy_command: "cargo publish".to_string(),
        dev_command: Some("cargo watch -x run".to_string()),
        confidence: 0.95,
        reasoning: "Found Cargo.toml with standard structure".to_string(),
        warnings: vec![],
        detected_files: vec!["Cargo.toml".to_string()],
        processing_time_ms: 1000,
    };

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format(&result).unwrap();

    // Verify JSON is valid
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["build_system"], "cargo");
    assert_eq!(parsed["language"], "Rust");
    assert_eq!(parsed["confidence"], 0.95);
}

#[test]
fn test_output_format_yaml() {
    let result = DetectionResult {
        build_system: "npm".to_string(),
        language: "JavaScript".to_string(),
        build_command: "npm run build".to_string(),
        test_command: "npm test".to_string(),
        deploy_command: "npm publish".to_string(),
        dev_command: None,
        confidence: 0.85,
        reasoning: "Found package.json".to_string(),
        warnings: vec!["No lock file found".to_string()],
        detected_files: vec!["package.json".to_string()],
        processing_time_ms: 800,
    };

    let formatter = OutputFormatter::new(OutputFormat::Yaml);
    let output = formatter.format(&result).unwrap();

    // Verify YAML is valid
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();
    assert_eq!(parsed["build_system"], "npm");
    assert_eq!(parsed["language"], "JavaScript");
}

#[test]
fn test_output_format_human_readable() {
    let result = DetectionResult {
        build_system: "cargo".to_string(),
        language: "Rust".to_string(),
        build_command: "cargo build --release".to_string(),
        test_command: "cargo test".to_string(),
        deploy_command: "cargo publish".to_string(),
        dev_command: None,
        confidence: 0.9,
        reasoning: "Standard Rust project".to_string(),
        warnings: vec![],
        detected_files: vec!["Cargo.toml".to_string()],
        processing_time_ms: 1200,
    };

    let formatter = OutputFormatter::new(OutputFormat::Human);
    let output = formatter.format(&result).unwrap();

    // Verify output contains key information
    assert!(output.contains("cargo"));
    assert!(output.contains("Rust"));
    assert!(output.contains("90%")); // No decimal in human format
    assert!(output.contains("cargo build --release"));
}

#[test]
fn test_detection_result_confidence_levels() {
    let mut result = DetectionResult::new(
        "cargo".to_string(),
        "Rust".to_string(),
        "cargo build".to_string(),
        "cargo test".to_string(),
        "cargo publish".to_string(),
    );

    result.set_confidence(0.95);
    assert_eq!(result.confidence_level(), "Very High");
    assert!(result.is_high_confidence());

    result.set_confidence(0.85);
    assert_eq!(result.confidence_level(), "High");

    result.set_confidence(0.5);
    assert_eq!(result.confidence_level(), "Very Low");
    assert!(result.is_low_confidence());
}

#[test]
fn test_detection_result_warnings() {
    let mut result = DetectionResult::new(
        "npm".to_string(),
        "JavaScript".to_string(),
        "npm run build".to_string(),
        "npm test".to_string(),
        "npm publish".to_string(),
    );

    assert!(!result.has_warnings());

    result.add_warning("No lock file found".to_string());
    assert!(result.has_warnings());
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn test_repository_context_builder() {
    let context = RepositoryContext::minimal(
        PathBuf::from("/test/repo"),
        "repo/\n├── file.txt".to_string(),
    )
    .with_key_file("Cargo.toml".to_string(), "[package]".to_string())
    .with_readme("# Test".to_string());

    assert_eq!(context.key_file_count(), 1);
    assert!(context.has_file("Cargo.toml"));
    assert!(context.readme_content.is_some());
}

#[tokio::test]
async fn test_analyzer_respects_ignore_patterns() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Create structure with ignored directories
    fs::create_dir(repo_path.join("node_modules")).unwrap();
    fs::write(repo_path.join("node_modules/package.json"), "{}").unwrap();
    fs::create_dir(repo_path.join("target")).unwrap();
    fs::write(repo_path.join("target/debug"), "binary").unwrap();
    fs::write(repo_path.join("main.js"), "console.log('hello')").unwrap();

    let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
    let context = analyzer.analyze().await.unwrap();

    // Should contain main.js but not ignored directories' contents
    assert!(context.file_tree.contains("main.js"));
    // Ignored directories should not be in the tree
    let tree_lines: Vec<&str> = context.file_tree.lines().collect();
    let has_node_modules_content = tree_lines.iter().any(|line| line.contains("package.json"));
    assert!(!has_node_modules_content);
}

#[test]
fn test_config_display_map() {
    let config = AipackConfig {
        backend: "ollama".to_string(),
        ollama_endpoint: "http://localhost:11434".to_string(),
        ollama_model: "qwen:7b".to_string(),
        mistral_api_key: Some("secret-key".to_string()),
        mistral_model: "mistral-small".to_string(),
        cache_enabled: true,
        cache_dir: Some(PathBuf::from("/tmp/cache")),
        request_timeout_secs: 30,
        max_context_size: 512_000,
        log_level: "info".to_string(),
    };

    // Without secrets
    let map = config.to_display_map(false);
    assert!(map["mistral_api_key"].contains("***"));

    // With secrets
    let map = config.to_display_map(true);
    assert_eq!(map["mistral_api_key"], "secret-key");
}

#[test]
fn test_help_output_generation() {
    use clap::CommandFactory;
    let mut cmd = CliArgs::command();
    let help = cmd.render_help().to_string();

    // Verify help contains key information
    assert!(help.contains("aipack"));
    assert!(help.contains("detect"));
    assert!(help.contains("health"));
    assert!(help.contains("config"));
}

#[test]
fn test_version_output() {
    use aipack::VERSION;
    assert!(!VERSION.is_empty());
    assert!(VERSION.contains('.'));
}

// Tests requiring Ollama are in ollama_integration.rs
// This file focuses on unit and integration tests that don't require external services
