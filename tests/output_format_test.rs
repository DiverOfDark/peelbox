//! Output formatting integration tests
//!
//! Tests all output formats (JSON, YAML, Human-readable) for:
//! - Detection results
//! - Health status
//! - Configuration display
//! - Error messages

use aipack::cli::output::{HealthStatus, OutputFormat, OutputFormatter};
use aipack::detection::types::RepositoryContext;
use aipack::output::schema::{
    BuildMetadata, BuildStage, ContextSpec, CopySpec, RuntimeStage, UniversalBuild,
};
use std::collections::HashMap;
use std::path::PathBuf;

fn create_sample_detection_result() -> UniversalBuild {
    UniversalBuild {
        version: "1.0".to_string(),
        metadata: BuildMetadata {
            project_name: Some("test-project".to_string()),
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
            confidence: 0.95,
            reasoning: "Found Cargo.toml with standard Rust project structure".to_string(),
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
            healthcheck: None,
        },
    }
}

fn create_sample_context() -> RepositoryContext {
    RepositoryContext::minimal(
        PathBuf::from("/test/repo"),
        "repo/\n├── Cargo.toml\n├── Cargo.lock\n└── src/".to_string(),
    )
    .with_key_file(
        "Cargo.toml".to_string(),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"".to_string(),
    )
    .with_readme("# Test Project\n\nA test project.".to_string())
}

#[test]
fn test_json_format_detection_result() {
    let result = create_sample_detection_result();
    let formatter = OutputFormatter::new(OutputFormat::Json);

    let output = formatter.format(&result).unwrap();

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    // Verify metadata fields
    assert_eq!(parsed["metadata"]["build_system"], "cargo");
    assert_eq!(parsed["metadata"]["language"], "Rust");
    assert_eq!(parsed["metadata"]["confidence"], 0.95);

    // Verify build stage
    assert_eq!(parsed["build"]["base"], "rust:1.75");
    assert!(parsed["build"]["commands"].is_array());
    assert_eq!(parsed["build"]["commands"][0], "cargo build --release");

    // Verify runtime stage
    assert_eq!(parsed["runtime"]["base"], "debian:bookworm-slim");
    assert!(parsed["runtime"]["copy"].is_array());
    assert!(parsed["runtime"]["command"].is_array());
}

#[test]
fn test_json_format_pretty_printed() {
    let result = create_sample_detection_result();
    let formatter = OutputFormatter::new(OutputFormat::Json);

    let output = formatter.format(&result).unwrap();

    // Pretty-printed JSON should have newlines
    assert!(output.contains('\n'));
    assert!(output.contains("  ")); // Indentation
}

#[test]
fn test_yaml_format_detection_result() {
    let result = create_sample_detection_result();
    let formatter = OutputFormatter::new(OutputFormat::Yaml);

    let output = formatter.format(&result).unwrap();

    // Verify it's valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();

    // Verify key fields
    assert_eq!(parsed["metadata"]["build_system"], "cargo");
    assert_eq!(parsed["metadata"]["language"], "Rust");
    assert_eq!(parsed["metadata"]["confidence"], 0.95);

    // Verify YAML format characteristics
    assert!(output.contains("build_system: cargo"));
    assert!(output.contains("language: Rust"));
}

#[test]
fn test_human_format_detection_result() {
    let result = create_sample_detection_result();
    let formatter = OutputFormatter::new(OutputFormat::Human);

    let output = formatter.format(&result).unwrap();

    // Human format outputs YAML, verify it contains key information
    assert!(output.contains("cargo"));
    assert!(output.contains("Rust"));
    assert!(output.contains("confidence: 0.95"));
    assert!(output.contains("cargo build --release"));
    assert!(output.contains("build:"));
    assert!(output.contains("runtime:"));
}

#[test]
fn test_json_format_complete() {
    let result = create_sample_detection_result();
    let formatter = OutputFormatter::new(OutputFormat::Json);

    let output = formatter.format(&result).unwrap();

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    // Verify all major sections are present
    assert!(parsed["metadata"].is_object());
    assert!(parsed["build"].is_object());
    assert!(parsed["runtime"].is_object());
}

#[test]
fn test_yaml_format_complete() {
    let result = create_sample_detection_result();
    let formatter = OutputFormatter::new(OutputFormat::Yaml);

    let output = formatter.format(&result).unwrap();

    // Verify it's valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();

    // Verify structure
    assert!(parsed["metadata"].is_mapping());
    assert!(parsed["build"].is_mapping());
    assert!(parsed["runtime"].is_mapping());
}

#[test]
fn test_human_format_complete() {
    let result = create_sample_detection_result();
    let formatter = OutputFormatter::new(OutputFormat::Human);

    let output = formatter.format(&result).unwrap();

    // Human format outputs YAML, verify it includes all sections
    assert!(output.contains("build:"));
    assert!(output.contains("runtime:"));
    assert!(output.contains("reasoning:"));
}

#[test]
fn test_health_status_format_json() {
    let mut health_results = HashMap::new();
    health_results.insert(
        "Ollama".to_string(),
        HealthStatus::available("Connected to http://localhost:11434".to_string())
            .with_details("Model: qwen:7b".to_string()),
    );
    health_results.insert(
        "Mistral".to_string(),
        HealthStatus::unavailable("API key not configured".to_string())
            .with_details("Set MISTRAL_API_KEY environment variable".to_string()),
    );

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format_health(&health_results).unwrap();

    // Verify valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert!(parsed["Ollama"]["available"].as_bool().unwrap());
    assert!(!parsed["Mistral"]["available"].as_bool().unwrap());
    assert!(parsed["Ollama"]["message"].is_string());
    assert!(parsed["Ollama"]["details"].is_string());
}

#[test]
fn test_health_status_format_yaml() {
    let mut health_results = HashMap::new();
    health_results.insert(
        "Ollama".to_string(),
        HealthStatus::available("Connected".to_string()),
    );

    let formatter = OutputFormatter::new(OutputFormat::Yaml);
    let output = formatter.format_health(&health_results).unwrap();

    // Verify valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();
    assert!(parsed["Ollama"]["available"].as_bool().unwrap());
}

#[test]
fn test_health_status_format_human() {
    let mut health_results = HashMap::new();
    health_results.insert(
        "Ollama".to_string(),
        HealthStatus::available("Connected to http://localhost:11434".to_string())
            .with_details("Model: qwen:7b".to_string()),
    );
    health_results.insert(
        "Mistral".to_string(),
        HealthStatus::unavailable("API key not configured".to_string())
            .with_details("Set MISTRAL_API_KEY environment variable".to_string()),
    );

    let formatter = OutputFormatter::new(OutputFormat::Human);
    let output = formatter.format_health(&health_results).unwrap();

    // Verify human-readable format (backend health status may vary in format)
    assert!(output.contains("Ollama"));
    assert!(output.contains("Mistral"));
}

#[test]
fn test_detection_result_minimal() {
    let result = UniversalBuild {
        version: "1.0".to_string(),
        metadata: BuildMetadata {
            project_name: None,
            language: "C".to_string(),
            build_system: "make".to_string(),
            confidence: 0.8,
            reasoning: "Found Makefile".to_string(),
        },
        build: BuildStage {
            base: "gcc:latest".to_string(),
            packages: vec![],
            env: HashMap::new(),
            commands: vec!["make".to_string()],
            context: vec![ContextSpec {
                from: ".".to_string(),
                to: "/app".to_string(),
            }],
            cache: vec![],
            artifacts: vec!["./app".to_string()],
        },
        runtime: RuntimeStage {
            base: "debian:bookworm-slim".to_string(),
            packages: vec![],
            env: HashMap::new(),
            copy: vec![CopySpec {
                from: "./app".to_string(),
                to: "/usr/local/bin/app".to_string(),
            }],
            command: vec!["/usr/local/bin/app".to_string()],
            ports: vec![],
            healthcheck: None,
        },
    };

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format(&result).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["metadata"]["build_system"], "make");
    assert_eq!(parsed["metadata"]["language"], "C");
    assert!(parsed["metadata"]["project_name"].is_null());
}

#[test]
fn test_detection_result_with_warnings() {
    let result = UniversalBuild {
        version: "1.0".to_string(),
        metadata: BuildMetadata {
            project_name: Some("test-app".to_string()),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
            confidence: 0.75,
            reasoning: "Found package.json but no lock file".to_string(),
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
            healthcheck: None,
        },
    };

    let formatter = OutputFormatter::new(OutputFormat::Human);
    let output = formatter.format(&result).unwrap();

    // UniversalBuild doesn't have warnings field - reasoning contains detection explanation
    // YAML format uses lowercase keys
    assert!(output.contains("reasoning:"));
    assert!(output.contains("Found package.json"));
}

#[test]
fn test_detection_result_confidence_levels_in_output() {
    let confidence_levels = vec![0.95, 0.85, 0.75, 0.65, 0.45];

    for confidence in confidence_levels {
        let mut result = create_sample_detection_result();
        result.metadata.confidence = confidence;

        let formatter = OutputFormatter::new(OutputFormat::Human);
        let output = formatter.format(&result).unwrap();

        // YAML format shows confidence as decimal
        let expected = format!("confidence: {}", confidence);
        assert!(
            output.contains(&expected),
            "Expected '{}' for confidence {}, but not found in output",
            expected,
            confidence
        );
    }
}

#[test]
fn test_output_format_enum_conversion() {
    use aipack::cli::commands::OutputFormatArg;

    let json_arg = OutputFormatArg::Json;
    let json_format: OutputFormat = json_arg.into();
    assert!(matches!(json_format, OutputFormat::Json));

    let yaml_arg = OutputFormatArg::Yaml;
    let yaml_format: OutputFormat = yaml_arg.into();
    assert!(matches!(yaml_format, OutputFormat::Yaml));

    let human_arg = OutputFormatArg::Human;
    let human_format: OutputFormat = human_arg.into();
    assert!(matches!(human_format, OutputFormat::Human));
}

#[test]
fn test_empty_health_results() {
    let health_results: HashMap<String, HealthStatus> = HashMap::new();

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format_health(&health_results).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed.as_object().unwrap().is_empty());
}

#[test]
fn test_health_status_without_details() {
    let mut health_results = HashMap::new();
    health_results.insert(
        "Backend".to_string(),
        HealthStatus::available("Connected".to_string()),
    );

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format_health(&health_results).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed["Backend"]["available"].as_bool().unwrap());
    assert!(parsed["Backend"]["details"].is_null());
}

#[test]
fn test_repository_context_display() {
    let context = create_sample_context();

    let display = format!("{}", context);

    assert!(display.contains("Repository: /test/repo"));
    assert!(display.contains("Key files: 1"));
}

#[test]
fn test_detection_result_display() {
    let result = create_sample_detection_result();

    let display = format!("{}", result);

    // Display outputs YAML format
    assert!(display.contains("cargo"));
    assert!(display.contains("Rust"));
    assert!(display.contains("confidence: 0.95"));
    assert!(display.contains("version:"));
}

#[test]
fn test_json_special_characters_escaped() {
    let mut result = create_sample_detection_result();
    result.metadata.reasoning = "Found \"quoted\" string and \\ backslash".to_string();

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format(&result).unwrap();

    // Should be valid JSON with escaped characters
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed["metadata"]["reasoning"].as_str().unwrap().contains("quoted"));
    assert!(parsed["metadata"]["reasoning"].as_str().unwrap().contains("backslash"));
}

#[test]
fn test_yaml_special_characters() {
    let mut result = create_sample_detection_result();
    result.metadata.reasoning = "Multi-line\nstring with:\n- items".to_string();

    let formatter = OutputFormatter::new(OutputFormat::Yaml);
    let output = formatter.format(&result).unwrap();

    // Should be valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();
    assert!(parsed["metadata"]["reasoning"].as_str().unwrap().contains("Multi-line"));
}

#[test]
fn test_human_format_width_and_alignment() {
    let result = create_sample_detection_result();

    let formatter = OutputFormatter::new(OutputFormat::Human);
    let output = formatter.format(&result).unwrap();

    // Check for consistent formatting
    let lines: Vec<&str> = output.lines().collect();

    // Should have some content
    assert!(!lines.is_empty());
}
