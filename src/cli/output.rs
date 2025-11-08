use anyhow::{Context, Result};
use serde_json;
use serde_yaml;
use std::collections::HashMap;

use crate::output::schema::UniversalBuild;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Yaml,
    Human,
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
            OutputFormat::Json => serde_json::to_string_pretty(result).context("Failed to serialize UniversalBuild to JSON"),
            OutputFormat::Yaml => result.to_yaml(),
            OutputFormat::Human => Ok(format!("{}", result)),
        }
    }

    pub fn format_health(&self, health_results: &HashMap<String, HealthStatus>) -> Result<String> {
        match self.format {
            OutputFormat::Json => {
                serde_json::to_string_pretty(health_results)
                    .context("Failed to serialize health status to JSON")
            }
            OutputFormat::Yaml => serde_yaml::to_string(health_results).context("Failed to serialize health status to YAML"),
            OutputFormat::Human => self.format_health_human(health_results),
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
            OutputFormat::Human => self.format_health_with_env_vars_human(health_results, env_vars),
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
    use crate::output::schema::{UniversalBuild, BuildMetadata, BuildStage, RuntimeStage, CopySpec};

    fn create_test_result() -> UniversalBuild {
        UniversalBuild {
            version: "1.0".to_string(),
            metadata: BuildMetadata {
                project_name: Some("test-app".to_string()),
                language: "rust".to_string(),
                build_system: "cargo".to_string(),
                confidence: 0.95,
                reasoning: "Detected Cargo.toml with standard Rust project structure".to_string(),
            },
            build: BuildStage {
                base: "rust:1.75".to_string(),
                packages: vec![],
                env: HashMap::new(),
                commands: vec!["cargo build --release".to_string()],
                context: vec![".".to_string(), "/app".to_string()],
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
    fn test_human_format() {
        let result = create_test_result();
        let formatter = OutputFormatter::new(OutputFormat::Human);
        let output = formatter.format(&result).unwrap();

        assert!(output.contains("Build System"));
        assert!(output.contains("cargo"));
        assert!(output.contains("rust"));
        assert!(output.contains("Confidence"));
        assert!(output.contains("95"));
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
