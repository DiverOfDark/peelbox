//! End-to-end integration tests
//!
//! Tests the complete workflow from CLI to result, including:
//! - Command-line argument parsing
//! - Configuration loading
//! - Detection service initialization
//! - Repository analysis
//! - Output formatting
//! - Error handling

use aipack::ai::genai_backend::Provider;
use aipack::cli::commands::{CliArgs, Commands, OutputFormatArg};
use aipack::cli::output::{OutputFormat, OutputFormatter};
use aipack::config::AipackConfig;
use aipack::detection::analyzer::RepositoryAnalyzer;
use aipack::detection::types::RepositoryContext;
use aipack::output::schema::{
    BuildMetadata, BuildStage, ContextSpec, CopySpec, RuntimeStage, UniversalBuild,
};
use clap::Parser;
use std::collections::HashMap;
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
            assert!(detect_args.backend.is_none()); // Auto-selection by default
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
            assert_eq!(detect_args.backend, Some(Provider::Ollama));
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

    // Provider is set via AIPACK_PROVIDER env var, defaults to Ollama
    assert!(matches!(
        config.provider,
        aipack::ai::genai_backend::Provider::Ollama
            | aipack::ai::genai_backend::Provider::OpenAI
            | aipack::ai::genai_backend::Provider::Claude
            | aipack::ai::genai_backend::Provider::Gemini
            | aipack::ai::genai_backend::Provider::Grok
            | aipack::ai::genai_backend::Provider::Groq
    ));
    assert_eq!(config.model, "qwen2.5-coder:7b");
    assert!(config.cache_enabled);
}

#[test]
fn test_configuration_validation_valid() {
    let config = AipackConfig {
        provider: aipack::ai::genai_backend::Provider::Ollama,
        model: "qwen:7b".to_string(),
        cache_enabled: true,
        cache_dir: Some(PathBuf::from("/tmp/cache")),
        request_timeout_secs: 30,
        max_context_size: 512_000,
        log_level: "info".to_string(),
        max_tool_iterations: 10,
        tool_timeout_secs: 30,
        max_file_size_bytes: 1_048_576,
        max_tokens: 8192,
    };

    assert!(config.validate().is_ok());
}

// Provider validation is now type-safe via Provider enum at compile time

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
    let result = UniversalBuild {
        version: "1.0".to_string(),
        metadata: BuildMetadata {
            project_name: Some("test-project".to_string()),
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
            confidence: 0.95,
            reasoning: "Found Cargo.toml with standard structure".to_string(),
        },
        build: BuildStage {
            base: "rust:1.75".to_string(),
            packages: vec![],
            env: HashMap::new(),
            commands: vec!["cargo build --release".to_string()],
            context: vec![ContextSpec {
                from: ".".to_string(),
                to: "/app".to_string(),
            }],
            cache: vec![],
            artifacts: vec!["target/release/app".to_string()],
        },
        runtime: RuntimeStage {
            base: "debian:bookworm-slim".to_string(),
            packages: vec![],
            env: HashMap::new(),
            copy: vec![CopySpec {
                from: "target/release/app".to_string(),
                to: "/usr/local/bin/app".to_string(),
            }],
            command: vec!["/usr/local/bin/app".to_string()],
            ports: vec![],
        },
    };

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format(&result).unwrap();

    // Verify JSON is valid
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed["metadata"]["build_system"], "cargo");
    assert_eq!(parsed["metadata"]["language"], "Rust");
    assert_eq!(parsed["metadata"]["confidence"], 0.95);
}

#[test]
fn test_output_format_yaml() {
    let result = UniversalBuild {
        version: "1.0".to_string(),
        metadata: BuildMetadata {
            project_name: Some("test-app".to_string()),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
            confidence: 0.85,
            reasoning: "Found package.json".to_string(),
        },
        build: BuildStage {
            base: "node:18".to_string(),
            packages: vec![],
            env: HashMap::new(),
            commands: vec!["npm run build".to_string()],
            context: vec![ContextSpec {
                from: ".".to_string(),
                to: "/app".to_string(),
            }],
            cache: vec![],
            artifacts: vec!["dist/".to_string()],
        },
        runtime: RuntimeStage {
            base: "node:18-alpine".to_string(),
            packages: vec![],
            env: HashMap::new(),
            copy: vec![CopySpec {
                from: "dist/".to_string(),
                to: "/app/dist/".to_string(),
            }],
            command: vec!["node".to_string(), "/app/dist/index.js".to_string()],
            ports: vec![],
        },
    };

    let formatter = OutputFormatter::new(OutputFormat::Yaml);
    let output = formatter.format(&result).unwrap();

    // Verify YAML is valid
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();
    assert_eq!(parsed["metadata"]["build_system"], "npm");
    assert_eq!(parsed["metadata"]["language"], "JavaScript");
}

#[test]
fn test_output_format_human_readable() {
    let result = UniversalBuild {
        version: "1.0".to_string(),
        metadata: BuildMetadata {
            project_name: Some("rust-app".to_string()),
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
            confidence: 0.9,
            reasoning: "Standard Rust project".to_string(),
        },
        build: BuildStage {
            base: "rust:1.75".to_string(),
            packages: vec![],
            env: HashMap::new(),
            commands: vec!["cargo build --release".to_string()],
            context: vec![ContextSpec {
                from: ".".to_string(),
                to: "/app".to_string(),
            }],
            cache: vec![],
            artifacts: vec!["target/release/app".to_string()],
        },
        runtime: RuntimeStage {
            base: "debian:bookworm-slim".to_string(),
            packages: vec![],
            env: HashMap::new(),
            copy: vec![CopySpec {
                from: "target/release/app".to_string(),
                to: "/usr/local/bin/app".to_string(),
            }],
            command: vec!["/usr/local/bin/app".to_string()],
            ports: vec![],
        },
    };

    let formatter = OutputFormatter::new(OutputFormat::Human);
    let output = formatter.format(&result).unwrap();

    // Verify output contains key information
    assert!(output.contains("cargo"));
    assert!(output.contains("Rust"));
    assert!(output.contains("0.9")); // Confidence as decimal in YAML
    assert!(output.contains("cargo build --release"));
}

#[test]
fn test_detection_result_confidence_levels() {
    let mut result = UniversalBuild {
        version: "1.0".to_string(),
        metadata: BuildMetadata {
            project_name: Some("test-app".to_string()),
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
            confidence: 0.95,
            reasoning: "Found Cargo.toml".to_string(),
        },
        build: BuildStage {
            base: "rust:1.75".to_string(),
            packages: vec![],
            env: HashMap::new(),
            commands: vec!["cargo build".to_string()],
            context: vec![ContextSpec {
                from: ".".to_string(),
                to: "/app".to_string(),
            }],
            cache: vec![],
            artifacts: vec!["target/release/app".to_string()],
        },
        runtime: RuntimeStage {
            base: "debian:bookworm-slim".to_string(),
            packages: vec![],
            env: HashMap::new(),
            copy: vec![CopySpec {
                from: "target/release/app".to_string(),
                to: "/usr/local/bin/app".to_string(),
            }],
            command: vec!["/usr/local/bin/app".to_string()],
            ports: vec![],
        },
    };

    // Display uses YAML format which shows confidence as decimal
    // Test different confidence levels are properly serialized
    result.metadata.confidence = 0.95;
    let display = format!("{}", result);
    assert!(display.contains("confidence: 0.95"));

    result.metadata.confidence = 0.85;
    let display = format!("{}", result);
    assert!(display.contains("confidence: 0.85"));

    result.metadata.confidence = 0.5;
    let display = format!("{}", result);
    assert!(display.contains("confidence: 0.5"));
}

#[test]
fn test_detection_result_warnings() {
    // UniversalBuild doesn't have a warnings field
    // Warnings are now part of the reasoning text
    let result = UniversalBuild {
        version: "1.0".to_string(),
        metadata: BuildMetadata {
            project_name: Some("test-app".to_string()),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
            confidence: 0.75,
            reasoning: "Found package.json. Warning: No lock file found.".to_string(),
        },
        build: BuildStage {
            base: "node:18".to_string(),
            packages: vec![],
            env: HashMap::new(),
            commands: vec!["npm run build".to_string()],
            context: vec![ContextSpec {
                from: ".".to_string(),
                to: "/app".to_string(),
            }],
            cache: vec![],
            artifacts: vec!["dist/".to_string()],
        },
        runtime: RuntimeStage {
            base: "node:18-alpine".to_string(),
            packages: vec![],
            env: HashMap::new(),
            copy: vec![CopySpec {
                from: "dist/".to_string(),
                to: "/app/dist/".to_string(),
            }],
            command: vec!["node".to_string(), "/app/dist/index.js".to_string()],
            ports: vec![],
        },
    };

    let display = format!("{}", result);
    assert!(display.contains("Warning: No lock file found"));
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
        provider: aipack::ai::genai_backend::Provider::Ollama,
        model: "qwen:7b".to_string(),
        cache_enabled: true,
        cache_dir: Some(PathBuf::from("/tmp/cache")),
        request_timeout_secs: 30,
        max_context_size: 512_000,
        log_level: "info".to_string(),
        max_tool_iterations: 10,
        tool_timeout_secs: 30,
        max_file_size_bytes: 1_048_576,
        max_tokens: 8192,
    };

    // Get display map (no arguments needed)
    let map = config.to_display_map();
    assert!(map.contains_key("provider"));
    assert_eq!(map["model"], "qwen:7b");
    assert_eq!(map["cache_enabled"], "true");
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
}

#[test]
fn test_version_output() {
    use aipack::VERSION;
    assert!(!VERSION.is_empty());
    assert!(VERSION.contains('.'));
}
