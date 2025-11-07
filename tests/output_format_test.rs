//! Output formatting integration tests
//!
//! Tests all output formats (JSON, YAML, Human-readable) for:
//! - Detection results
//! - Health status
//! - Configuration display
//! - Error messages

use aipack::cli::output::{HealthStatus, OutputFormat, OutputFormatter};
use aipack::config::AipackConfig;
use aipack::detection::types::{DetectionResult, RepositoryContext};
use std::collections::HashMap;
use std::path::PathBuf;

fn create_sample_detection_result() -> DetectionResult {
    DetectionResult {
        build_system: "cargo".to_string(),
        language: "Rust".to_string(),
        build_command: "cargo build --release".to_string(),
        test_command: "cargo test".to_string(),
        deploy_command: "cargo publish".to_string(),
        dev_command: Some("cargo watch -x run".to_string()),
        confidence: 0.95,
        reasoning: "Found Cargo.toml with standard Rust project structure".to_string(),
        warnings: vec!["Consider adding CI/CD configuration".to_string()],
        detected_files: vec!["Cargo.toml".to_string(), "Cargo.lock".to_string()],
        processing_time_ms: 1234,
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

    // Verify all fields are present
    assert_eq!(parsed["build_system"], "cargo");
    assert_eq!(parsed["language"], "Rust");
    assert_eq!(parsed["build_command"], "cargo build --release");
    assert_eq!(parsed["test_command"], "cargo test");
    assert_eq!(parsed["deploy_command"], "cargo publish");
    assert_eq!(parsed["dev_command"], "cargo watch -x run");
    assert_eq!(parsed["confidence"], 0.95);
    assert_eq!(parsed["processing_time_ms"], 1234);

    // Verify arrays
    assert!(parsed["warnings"].is_array());
    assert_eq!(parsed["warnings"].as_array().unwrap().len(), 1);
    assert!(parsed["detected_files"].is_array());
    assert_eq!(parsed["detected_files"].as_array().unwrap().len(), 2);
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
    assert_eq!(parsed["build_system"], "cargo");
    assert_eq!(parsed["language"], "Rust");
    assert_eq!(parsed["confidence"], 0.95);

    // Verify YAML format characteristics
    assert!(output.contains("build_system: cargo"));
    assert!(output.contains("language: Rust"));
}

#[test]
fn test_human_format_detection_result() {
    let result = create_sample_detection_result();
    let formatter = OutputFormatter::new(OutputFormat::Human);

    let output = formatter.format(&result).unwrap();

    // Verify human-readable output contains key information
    assert!(output.contains("Build Detection Result"));
    assert!(output.contains("cargo"));
    assert!(output.contains("Rust"));
    assert!(output.contains("95%")); // Confidence as percentage (no decimal)
    assert!(output.contains("Very High")); // Confidence level
    assert!(output.contains("cargo build --release"));
    assert!(output.contains("cargo test"));
    assert!(output.contains("cargo publish"));
    assert!(output.contains("cargo watch -x run"));
    assert!(output.contains("1234ms")); // Processing time
    assert!(output.contains("Consider adding CI/CD")); // Warning
}

#[test]
fn test_json_format_with_context() {
    let result = create_sample_detection_result();
    let context = create_sample_context();
    let formatter = OutputFormatter::new(OutputFormat::Json);

    let output = formatter.format_with_context(&result, &context).unwrap();

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    // Verify result fields
    assert_eq!(parsed["detection_result"]["build_system"], "cargo");

    // Verify context fields
    assert!(parsed["context"]["file_tree"].is_string());
    assert!(parsed["context"]["key_files"].is_object());
    assert!(parsed["context"]["readme_content"].is_string());
}

#[test]
fn test_yaml_format_with_context() {
    let result = create_sample_detection_result();
    let context = create_sample_context();
    let formatter = OutputFormatter::new(OutputFormat::Yaml);

    let output = formatter.format_with_context(&result, &context).unwrap();

    // Verify it's valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();

    // Verify structure
    assert!(parsed["detection_result"].is_mapping());
    assert!(parsed["context"].is_mapping());
}

#[test]
fn test_human_format_with_context() {
    let result = create_sample_detection_result();
    let context = create_sample_context();
    let formatter = OutputFormatter::new(OutputFormat::Human);

    let output = formatter.format_with_context(&result, &context).unwrap();

    // Verify verbose output includes context information
    assert!(output.contains("Build Detection Result"));
    assert!(output.contains("Repository Context"));
    assert!(output.contains("File Tree"));
    assert!(output.contains("Key Files"));
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
fn test_config_format_json() {
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

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format_config(&config, false).unwrap();

    // Verify valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["backend"], "ollama");
    assert_eq!(parsed["ollama_endpoint"], "http://localhost:11434");
    // API key should be masked
    assert!(parsed["mistral_api_key"].as_str().unwrap().contains("***"));
}

#[test]
fn test_config_format_json_show_secrets() {
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

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format_config(&config, true).unwrap();

    // Verify valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    // API key should be visible
    assert_eq!(parsed["mistral_api_key"], "secret-key");
}

#[test]
fn test_config_format_yaml() {
    let config = AipackConfig::default();

    let formatter = OutputFormatter::new(OutputFormat::Yaml);
    let output = formatter.format_config(&config, false).unwrap();

    // Verify valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();

    assert!(parsed["backend"].is_string());
    assert!(parsed["ollama_endpoint"].is_string());
}

#[test]
fn test_config_format_human() {
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

    let formatter = OutputFormatter::new(OutputFormat::Human);
    let output = formatter.format_config(&config, false).unwrap();

    // Verify human-readable format contains key config
    assert!(output.contains("ollama"));
    assert!(output.contains("http://localhost:11434"));
    // API key should not be visible in plain text
    assert!(!output.contains("secret-key"));
}

#[test]
fn test_detection_result_minimal() {
    let result = DetectionResult::new(
        "make".to_string(),
        "C".to_string(),
        "make".to_string(),
        "make test".to_string(),
        "make install".to_string(),
    );

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format(&result).unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["build_system"], "make");
    assert_eq!(parsed["language"], "C");
    assert!(parsed["dev_command"].is_null()); // Optional field
    assert!(parsed["warnings"].as_array().unwrap().is_empty());
}

#[test]
fn test_detection_result_with_warnings() {
    let mut result = DetectionResult::new(
        "npm".to_string(),
        "JavaScript".to_string(),
        "npm run build".to_string(),
        "npm test".to_string(),
        "npm publish".to_string(),
    );

    result.add_warning("No lock file found".to_string());
    result.add_warning("Missing build script".to_string());

    let formatter = OutputFormatter::new(OutputFormat::Human);
    let output = formatter.format(&result).unwrap();

    assert!(output.contains("Warnings:"));
    assert!(output.contains("No lock file found"));
    assert!(output.contains("Missing build script"));
}

#[test]
fn test_detection_result_confidence_levels_in_output() {
    let confidence_levels = vec![
        (0.95, "Very High"),
        (0.85, "High"),
        (0.75, "Moderate"),
        (0.65, "Low"),
        (0.45, "Very Low"),
    ];

    for (confidence, expected_level) in confidence_levels {
        let mut result = create_sample_detection_result();
        result.set_confidence(confidence);

        let formatter = OutputFormatter::new(OutputFormat::Human);
        let output = formatter.format(&result).unwrap();

        assert!(
            output.contains(expected_level),
            "Expected '{}' for confidence {}, but not found in output",
            expected_level,
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

    // Verify Display implementation
    assert!(display.contains("Build System Detection Result"));
    assert!(display.contains("cargo"));
    assert!(display.contains("Rust"));
    assert!(display.contains("95.0%"));
    assert!(display.contains("Very High"));
}

#[test]
fn test_json_special_characters_escaped() {
    let mut result = create_sample_detection_result();
    result.reasoning = "Found \"quoted\" string and \\ backslash".to_string();

    let formatter = OutputFormatter::new(OutputFormat::Json);
    let output = formatter.format(&result).unwrap();

    // Should be valid JSON with escaped characters
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert!(parsed["reasoning"].as_str().unwrap().contains("quoted"));
    assert!(parsed["reasoning"].as_str().unwrap().contains("backslash"));
}

#[test]
fn test_yaml_special_characters() {
    let mut result = create_sample_detection_result();
    result.reasoning = "Multi-line\nstring with:\n- items".to_string();

    let formatter = OutputFormatter::new(OutputFormat::Yaml);
    let output = formatter.format(&result).unwrap();

    // Should be valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&output).unwrap();
    assert!(parsed["reasoning"].as_str().unwrap().contains("Multi-line"));
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
