use anyhow::{Context, Result};
use serde_json;
use serde_yaml;
use std::collections::HashMap;

use crate::output::schema::UniversalBuild;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Yaml,
}

pub struct OutputFormatter {
    format: OutputFormat,
}

impl OutputFormatter {
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    pub fn format(&self, result: &UniversalBuild) -> Result<String> {
        match self.format {
            OutputFormat::Json => serde_json::to_string_pretty(result)
                .context("Failed to serialize UniversalBuild to JSON"),
            OutputFormat::Yaml => result.to_yaml(),
        }
    }

    pub fn format_multiple(&self, results: &[UniversalBuild]) -> Result<String> {
        match self.format {
            OutputFormat::Json => serde_json::to_string_pretty(results)
                .context("Failed to serialize UniversalBuild array to JSON"),
            OutputFormat::Yaml => serde_yaml::to_string(results)
                .context("Failed to serialize UniversalBuild array to YAML"),
        }
    }

    pub fn format_health(&self, health_results: &HashMap<String, HealthStatus>) -> Result<String> {
        match self.format {
            OutputFormat::Json => serde_json::to_string_pretty(health_results)
                .context("Failed to serialize health status to JSON"),
            OutputFormat::Yaml => serde_yaml::to_string(health_results)
                .context("Failed to serialize health status to YAML"),
        }
    }

    pub fn format_health_with_env_vars(
        &self,
        health_results: &HashMap<String, HealthStatus>,
        env_vars: &HashMap<String, Vec<EnvVarInfo>>,
    ) -> Result<String> {
        match self.format {
            OutputFormat::Json => self.format_health_with_env_vars_json(health_results, env_vars),
            OutputFormat::Yaml => self.format_health_with_env_vars_yaml(health_results, env_vars),
        }
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
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HealthStatus {
    pub available: bool,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnvVarInfo {
    pub name: String,
    pub value: Option<String>,
    pub default: Option<String>,
    pub required: bool,
    pub description: String,
}

impl HealthStatus {
    pub fn available(message: String) -> Self {
        Self {
            available: true,
            message,
            details: None,
        }
    }

    pub fn unavailable(message: String) -> Self {
        Self {
            available: false,
            message,
            details: None,
        }
    }

    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::schema::{
        BuildMetadata, BuildStage, ContextSpec, CopySpec, RuntimeStage, UniversalBuild,
    };

    fn create_test_result() -> UniversalBuild {
        UniversalBuild {
            version: "1.0".to_string(),
            metadata: BuildMetadata {
                project_name: Some("test-app".to_string()),
                language: "rust".to_string(),
                build_system: "cargo".to_string(),
                framework: None,
                confidence: 0.95,
                reasoning: "Detected Cargo.toml with standard Rust project structure".to_string(),
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
                health: None,
            },
        }
    }

    #[test]
    fn test_json_format() {
        let result = create_test_result();
        let formatter = OutputFormatter::new(OutputFormat::Json);
        let output = formatter.format(&result).unwrap();

        assert!(output.contains("cargo"));
        assert!(output.contains("rust"));
        assert!(output.contains("0.95"));

        // Verify it's valid JSON
        let _parsed: UniversalBuild = serde_json::from_str(&output).unwrap();
    }

    #[test]
    fn test_yaml_format() {
        let result = create_test_result();
        let formatter = OutputFormatter::new(OutputFormat::Yaml);
        let output = formatter.format(&result).unwrap();

        assert!(output.contains("cargo"));
        assert!(output.contains("rust"));
        assert!(output.contains("0.95"));

        // Verify it's valid YAML
        let _parsed: UniversalBuild = serde_yaml::from_str(&output).unwrap();
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
}
