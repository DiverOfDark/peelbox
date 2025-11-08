//! Output formatting for multiple formats
//!
//! This module provides formatters for different output formats including JSON, YAML,
//! and human-readable text. Each formatter implements consistent styling and structure.
//!
//! # Example
//!
//! ```ignore
//! use aipack::cli::output::{OutputFormat, OutputFormatter};
//! use aipack::detection::types::DetectionResult;
//!
//! let result = DetectionResult::new(/* ... */);
//! let formatter = OutputFormatter::new(OutputFormat::Json);
//! let output = formatter.format(&result)?;
//! println!("{}", output);
//! ```

use anyhow::{Context, Result};
use serde_json;
use serde_yaml;
use std::collections::HashMap;

use crate::config::AipackConfig;
use crate::detection::types::{DetectionResult, RepositoryContext};

/// Output format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// JSON format (machine-readable)
    Json,
    /// YAML format (human-friendly, version-control friendly)
    Yaml,
    /// Human-readable formatted text
    Human,
}

/// Output formatter for detection results
pub struct OutputFormatter {
    format: OutputFormat,
}

impl OutputFormatter {
    /// Creates a new output formatter with the specified format
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    /// Formats a detection result according to the configured format
    pub fn format(&self, result: &DetectionResult) -> Result<String> {
        match self.format {
            OutputFormat::Json => self.format_json(result),
            OutputFormat::Yaml => self.format_yaml(result),
            OutputFormat::Human => self.format_human(result),
        }
    }

    /// Formats a detection result with repository context (verbose mode)
    pub fn format_with_context(
        &self,
        result: &DetectionResult,
        context: &RepositoryContext,
    ) -> Result<String> {
        match self.format {
            OutputFormat::Json => self.format_json_with_context(result, context),
            OutputFormat::Yaml => self.format_yaml_with_context(result, context),
            OutputFormat::Human => self.format_human_with_context(result, context),
        }
    }

    /// Formats configuration display
    pub fn format_config(&self, config: &AipackConfig) -> Result<String> {
        match self.format {
            OutputFormat::Json => self.format_config_json(config),
            OutputFormat::Yaml => self.format_config_yaml(config),
            OutputFormat::Human => self.format_config_human(config),
        }
    }

    /// Formats health check results
    pub fn format_health(&self, health_results: &HashMap<String, HealthStatus>) -> Result<String> {
        match self.format {
            OutputFormat::Json => self.format_health_json(health_results),
            OutputFormat::Yaml => self.format_health_yaml(health_results),
            OutputFormat::Human => self.format_health_human(health_results),
        }
    }

    /// Formats health check results with environment variable information
    pub fn format_health_with_env_vars(
        &self,
        health_results: &HashMap<String, HealthStatus>,
        env_vars: &HashMap<String, Vec<EnvVarInfo>>,
    ) -> Result<String> {
        match self.format {
            OutputFormat::Json => self.format_health_with_env_vars_json(health_results, env_vars),
            OutputFormat::Yaml => self.format_health_with_env_vars_yaml(health_results, env_vars),
            OutputFormat::Human => self.format_health_with_env_vars_human(health_results, env_vars),
        }
    }

    // JSON formatting methods

    fn format_json(&self, result: &DetectionResult) -> Result<String> {
        serde_json::to_string_pretty(result).context("Failed to serialize detection result to JSON")
    }

    fn format_json_with_context(
        &self,
        result: &DetectionResult,
        context: &RepositoryContext,
    ) -> Result<String> {
        let output = serde_json::json!({
            "detection_result": result,
            "context": {
                "repository_path": context.repo_path,
                "file_tree": context.file_tree,
                "key_files": context.key_files,
                "readme_content": context.readme_content,
                "detected_files": context.detected_files,
                "git_info": context.git_info,
            }
        });

        serde_json::to_string_pretty(&output)
            .context("Failed to serialize result with context to JSON")
    }

    fn format_config_json(&self, config: &AipackConfig) -> Result<String> {
        let config_map = config.to_display_map();
        serde_json::to_string_pretty(&config_map).context("Failed to serialize config to JSON")
    }

    fn format_health_json(&self, health_results: &HashMap<String, HealthStatus>) -> Result<String> {
        serde_json::to_string_pretty(health_results)
            .context("Failed to serialize health status to JSON")
    }

    fn format_health_with_env_vars_json(
        &self,
        health_results: &HashMap<String, HealthStatus>,
        env_vars: &HashMap<String, Vec<EnvVarInfo>>,
    ) -> Result<String> {
        let output = serde_json::json!({
            "health_status": health_results,
            "environment_variables": env_vars,
        });
        serde_json::to_string_pretty(&output)
            .context("Failed to serialize health status with env vars to JSON")
    }

    // YAML formatting methods

    fn format_yaml(&self, result: &DetectionResult) -> Result<String> {
        serde_yaml::to_string(result).context("Failed to serialize detection result to YAML")
    }

    fn format_yaml_with_context(
        &self,
        result: &DetectionResult,
        context: &RepositoryContext,
    ) -> Result<String> {
        let output = serde_json::json!({
            "detection_result": result,
            "context": {
                "repository_path": context.repo_path,
                "file_tree": context.file_tree,
                "key_files": context.key_files,
                "readme_content": context.readme_content,
                "detected_files": context.detected_files,
                "git_info": context.git_info,
            }
        });

        serde_yaml::to_string(&output).context("Failed to serialize result with context to YAML")
    }

    fn format_config_yaml(&self, config: &AipackConfig) -> Result<String> {
        let config_map = config.to_display_map();
        serde_yaml::to_string(&config_map).context("Failed to serialize config to YAML")
    }

    fn format_health_yaml(&self, health_results: &HashMap<String, HealthStatus>) -> Result<String> {
        serde_yaml::to_string(health_results).context("Failed to serialize health status to YAML")
    }

    fn format_health_with_env_vars_yaml(
        &self,
        health_results: &HashMap<String, HealthStatus>,
        env_vars: &HashMap<String, Vec<EnvVarInfo>>,
    ) -> Result<String> {
        let output = serde_json::json!({
            "health_status": health_results,
            "environment_variables": env_vars,
        });
        serde_yaml::to_string(&output)
            .context("Failed to serialize health status with env vars to YAML")
    }

    // Human-readable formatting methods

    fn format_human(&self, result: &DetectionResult) -> Result<String> {
        let mut output = String::new();

        // Header with check mark or warning
        if result.is_high_confidence() {
            output.push_str("\u{2713} Build Detection Result\n");
        } else {
            output.push_str("\u{26A0} Build Detection Result (Low Confidence)\n");
        }
        output.push_str("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\n");

        // Build system and language
        output.push_str(&format!("Build System:  {}\n", result.build_system));
        output.push_str(&format!("Language:      {}\n\n", result.language));

        // Build commands section
        output.push_str("Build Information:\n");
        output.push_str(&format!(
            "\u{251C}\u{2500} Build:   {}\n",
            result.build_command
        ));
        output.push_str(&format!(
            "\u{251C}\u{2500} Test:    {}\n",
            result.test_command
        ));
        if let Some(ref dev_cmd) = result.dev_command {
            output.push_str(&format!("\u{2514}\u{2500} Dev:     {}\n", dev_cmd));
        } else {
            output.push_str("\u{2514}\u{2500} Dev:     (not specified)\n");
        }
        output.push_str("\n");

        // Docker information section
        output.push_str("Docker Information:\n");
        output.push_str(&format!(
            "\u{251C}\u{2500} Runtime:      {}\n",
            result.runtime
        ));
        output.push_str(&format!(
            "\u{251C}\u{2500} Entry Point:  {}\n",
            result.entry_point
        ));
        if !result.dependencies.is_empty() {
            output.push_str("\u{251C}\u{2500} Dependencies:\n");
            for (i, dep) in result.dependencies.iter().enumerate() {
                let is_last = i == result.dependencies.len() - 1;
                let connector = if is_last { "\u{2514}" } else { "\u{251C}" };
                output.push_str(&format!("{}  \u{2500} {}\n", connector, dep));
            }
            output.push_str("\n");
        } else {
            output.push_str("\u{2514}\u{2500} Dependencies: (none specified)\n\n");
        }

        // Confidence bar
        let confidence_pct = (result.confidence * 100.0) as u8;
        let filled_blocks = (result.confidence * 10.0) as usize;
        let empty_blocks = 10 - filled_blocks;
        let confidence_bar = "\u{2588}".repeat(filled_blocks) + &"\u{2591}".repeat(empty_blocks);

        output.push_str(&format!(
            "Confidence: {} {}% ({})\n\n",
            confidence_bar,
            confidence_pct,
            result.confidence_level()
        ));

        // Detection summary
        output.push_str("Detection Summary:\n");
        if !result.detected_files.is_empty() {
            output.push_str(&format!("Files: {}\n", result.detected_files.join(", ")));
        }
        output.push_str(&format!("Reasoning: {}\n", result.reasoning));

        // Warnings if any
        if !result.warnings.is_empty() {
            output.push_str("\n\u{26A0} Warnings:\n");
            for warning in &result.warnings {
                output.push_str(&format!("  - {}\n", warning));
            }
        }

        // Processing time
        output.push_str(&format!("\nProcessed in {}ms\n", result.processing_time_ms));

        Ok(output)
    }

    fn format_human_with_context(
        &self,
        result: &DetectionResult,
        context: &RepositoryContext,
    ) -> Result<String> {
        let mut output = self.format_human(result)?;

        // Add context information
        output.push_str("\n\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n");
        output.push_str("Repository Context (Verbose)\n");
        output.push_str("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\n");

        output.push_str(&format!("Repository: {}\n\n", context.repo_path.display()));

        if let Some(ref git_info) = context.git_info {
            output.push_str(&format!(
                "Git: {} ({})\n\n",
                git_info.branch, git_info.commit_hash
            ));
        }

        output.push_str("File Tree:\n");
        output.push_str(&context.file_tree);
        output.push_str("\n\n");

        if !context.key_files.is_empty() {
            output.push_str("Key Files:\n");
            for (path, content) in &context.key_files {
                output.push_str(&format!("\n--- {} ---\n", path));
                let preview = if content.len() > 500 {
                    format!("{}... (truncated)", &content[..500])
                } else {
                    content.clone()
                };
                output.push_str(&preview);
                output.push('\n');
            }
        }

        Ok(output)
    }

    fn format_config_human(&self, config: &AipackConfig) -> Result<String> {
        let mut output = String::new();

        output.push_str("aipack Configuration\n");
        output.push_str("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\n");

        let config_map = config.to_display_map();

        // Backend section
        output.push_str("Backend Configuration:\n");
        if let Some(backend) = config_map.get("backend") {
            output.push_str(&format!("  Backend: {}\n", backend));
        }

        // Ollama section
        output.push_str("\nOllama Configuration:\n");
        if let Some(endpoint) = config_map.get("ollama_endpoint") {
            output.push_str(&format!("  Endpoint: {}\n", endpoint));
        }
        if let Some(model) = config_map.get("model") {
            output.push_str(&format!("  Model: {}\n", model));
        }
        if let Some(timeout) = config_map.get("ollama_timeout") {
            output.push_str(&format!("  Timeout: {}s\n", timeout));
        }

        // LM Studio section
        output.push_str("\nLM Studio Configuration:\n");
        if let Some(endpoint) = config_map.get("lm_studio_endpoint") {
            output.push_str(&format!("  Endpoint: {}\n", endpoint));
        }

        // Mistral section
        output.push_str("\nMistral Configuration:\n");
        if let Some(api_key) = config_map.get("mistral_api_key") {
            output.push_str(&format!("  API Key: {}\n", api_key));
        }
        if let Some(model) = config_map.get("mistral_model") {
            output.push_str(&format!("  Model: {}\n", model));
        }
        if let Some(timeout) = config_map.get("mistral_timeout") {
            output.push_str(&format!("  Timeout: {}s\n", timeout));
        }

        // Cache section
        output.push_str("\nCache Configuration:\n");
        if let Some(enabled) = config_map.get("cache_enabled") {
            output.push_str(&format!("  Enabled: {}\n", enabled));
        }

        Ok(output)
    }

    fn format_health_human(
        &self,
        health_results: &HashMap<String, HealthStatus>,
    ) -> Result<String> {
        let mut output = String::new();

        output.push_str("Backend Health Status\n");
        output.push_str("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\n");

        for (backend, status) in health_results {
            let status_symbol = if status.available {
                "\u{2713}"
            } else {
                "\u{2717}"
            };

            output.push_str(&format!("{} {}\n", status_symbol, backend));
            output.push_str(&format!(
                "  Status: {}\n",
                if status.available {
                    "Available"
                } else {
                    "Unavailable"
                }
            ));
            output.push_str(&format!("  Message: {}\n", status.message));

            if let Some(ref details) = status.details {
                output.push_str(&format!("  Details: {}\n", details));
            }
            output.push('\n');
        }

        Ok(output)
    }

    fn format_health_with_env_vars_human(
        &self,
        health_results: &HashMap<String, HealthStatus>,
        env_vars: &HashMap<String, Vec<EnvVarInfo>>,
    ) -> Result<String> {
        let mut output = self.format_health_human(health_results)?;

        // Add environment variables section
        output.push_str("Environment Variables\n");
        output.push_str("\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\u{2501}\n\n");

        // Sort backends for consistent output
        let mut backends: Vec<_> = env_vars.keys().collect();
        backends.sort();

        for backend in backends {
            if let Some(vars) = env_vars.get(backend) {
                output.push_str(&format!("{}:\n", backend));
                for var in vars {
                    let required_marker = if var.required { "*" } else { " " };
                    output.push_str(&format!("  {} {}\n", required_marker, var.name));

                    // Show current value
                    if let Some(ref value) = var.value {
                        output.push_str(&format!("    Current: {}\n", value));
                    } else {
                        output.push_str("    Current: not set\n");
                    }

                    // Show default if available
                    if let Some(ref default) = var.default {
                        output.push_str(&format!("    Default: {}\n", default));
                    }

                    // Show description
                    output.push_str(&format!("    Info: {}\n", var.description));
                }
                output.push('\n');
            }
        }

        output.push_str("* = required\n");

        Ok(output)
    }
}

/// Health status for a backend
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthStatus {
    /// Whether the backend is available
    pub available: bool,
    /// Status message
    pub message: String,
    /// Optional additional details
    pub details: Option<String>,
}

/// Environment variable information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnvVarInfo {
    /// Variable name
    pub name: String,
    /// Current value (masked for secrets)
    pub value: Option<String>,
    /// Default value if not set
    pub default: Option<String>,
    /// Whether this is a required variable
    pub required: bool,
    /// Description of what this variable does
    pub description: String,
}

impl HealthStatus {
    /// Creates a new health status indicating availability
    pub fn available(message: String) -> Self {
        Self {
            available: true,
            message,
            details: None,
        }
    }

    /// Creates a new health status indicating unavailability
    pub fn unavailable(message: String) -> Self {
        Self {
            available: false,
            message,
            details: None,
        }
    }

    /// Adds additional details to the health status
    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::types::DetectionResult;

    fn create_test_result() -> DetectionResult {
        DetectionResult {
            build_system: "cargo".to_string(),
            language: "Rust".to_string(),
            build_command: "cargo build --release".to_string(),
            test_command: "cargo test".to_string(),
            runtime: "rust:1.75".to_string(),
            dependencies: vec![],
            entry_point: "/app".to_string(),
            dev_command: Some("cargo watch -x run".to_string()),
            confidence: 0.95,
            reasoning: "Detected Cargo.toml with standard Rust project structure".to_string(),
            warnings: vec!["Consider adding CI/CD".to_string()],
            detected_files: vec!["Cargo.toml".to_string(), "Cargo.lock".to_string()],
            processing_time_ms: 1234,
        }
    }

    #[test]
    fn test_json_format() {
        let result = create_test_result();
        let formatter = OutputFormatter::new(OutputFormat::Json);
        let output = formatter.format(&result).unwrap();

        assert!(output.contains("cargo"));
        assert!(output.contains("Rust"));
        assert!(output.contains("0.95"));

        // Verify it's valid JSON
        let _parsed: DetectionResult = serde_json::from_str(&output).unwrap();
    }

    #[test]
    fn test_yaml_format() {
        let result = create_test_result();
        let formatter = OutputFormatter::new(OutputFormat::Yaml);
        let output = formatter.format(&result).unwrap();

        assert!(output.contains("cargo"));
        assert!(output.contains("Rust"));
        assert!(output.contains("0.95"));

        // Verify it's valid YAML
        let _parsed: DetectionResult = serde_yaml::from_str(&output).unwrap();
    }

    #[test]
    fn test_human_format() {
        let result = create_test_result();
        let formatter = OutputFormatter::new(OutputFormat::Human);
        let output = formatter.format(&result).unwrap();

        assert!(output.contains("Build System"));
        assert!(output.contains("cargo"));
        assert!(output.contains("Rust"));
        assert!(output.contains("Build Information:"));  // Docker-focused output
        assert!(output.contains("Docker Information:"));  // Docker-focused output
        assert!(output.contains("Runtime:"));  // Docker runtime
        assert!(output.contains("Entry Point:"));  // Container entry point
        assert!(output.contains("Confidence:"));
        assert!(output.contains("95%"));
        assert!(output.contains("Warnings:"));
        assert!(output.contains("1234ms"));
    }

    #[test]
    fn test_health_status_creation() {
        let status = HealthStatus::available("Ollama is running".to_string());
        assert!(status.available);
        assert_eq!(status.message, "Ollama is running");

        let status = HealthStatus::unavailable("Cannot connect".to_string())
            .with_details("Connection refused on localhost:11434".to_string());
        assert!(!status.available);
        assert!(status.details.is_some());
    }

    #[test]
    fn test_health_format_human() {
        let mut health_results = HashMap::new();
        health_results.insert(
            "Ollama".to_string(),
            HealthStatus::available("Connected successfully".to_string()),
        );
        health_results.insert(
            "Mistral".to_string(),
            HealthStatus::unavailable("API key not configured".to_string()),
        );

        let formatter = OutputFormatter::new(OutputFormat::Human);
        let output = formatter.format_health(&health_results).unwrap();

        assert!(output.contains("Backend Health Status"));
        assert!(output.contains("Ollama"));
        assert!(output.contains("Mistral"));
        assert!(output.contains("Available"));
        assert!(output.contains("Unavailable"));
    }
}
