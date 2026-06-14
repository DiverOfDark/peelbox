use anyhow::{Context, Result};
use serde_json::{self, Map, Value};

use peelbox_core::output::schema::UniversalBuild;

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
            OutputFormat::Json => {
                let value = serde_json::to_value(result)
                    .context("Failed to convert UniversalBuild to JSON value")?;
                let sorted = Self::sort_json_keys(value);
                serde_json::to_string_pretty(&sorted)
                    .context("Failed to serialize UniversalBuild to JSON")
            }
            OutputFormat::Yaml => result.to_yaml(),
        }
    }

    pub fn format_multiple(&self, results: &[UniversalBuild]) -> Result<String> {
        match self.format {
            OutputFormat::Json => {
                let value = serde_json::to_value(results)
                    .context("Failed to convert UniversalBuild array to JSON value")?;
                let sorted = Self::sort_json_keys(value);
                serde_json::to_string_pretty(&sorted)
                    .context("Failed to serialize UniversalBuild array to JSON")
            }
            OutputFormat::Yaml => serde_yaml::to_string(results)
                .context("Failed to serialize UniversalBuild array to YAML"),
        }
    }

    fn sort_json_keys(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut sorted = Map::new();
                let mut keys: Vec<_> = map.keys().cloned().collect();
                keys.sort();
                for key in keys {
                    if let Some(v) = map.get(&key) {
                        sorted.insert(key, Self::sort_json_keys(v.clone()));
                    }
                }
                Value::Object(sorted)
            }
            Value::Array(arr) => Value::Array(arr.into_iter().map(Self::sort_json_keys).collect()),
            _ => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peelbox_core::output::schema::{
        BuildMetadata, BuildStage, CopySpec, RuntimeStage, UniversalBuild,
    };
    use std::collections::BTreeMap;

    fn create_test_result() -> UniversalBuild {
        UniversalBuild {
            version: "1.0".to_string(),
            metadata: BuildMetadata {
                project_name: Some("test-app".to_string()),
                language: "rust".to_string(),
                build_system: "cargo".to_string(),
                framework: None,
                reasoning: "Detected Cargo.toml with standard Rust project structure".to_string(),
            },
            build: BuildStage {
                packages: vec!["rust".to_string(), "build-base".to_string()],
                env: BTreeMap::new(),
                commands: vec!["cargo build --release".to_string()],
                cache: vec![],
            },
            runtime: RuntimeStage {
                packages: vec!["glibc".to_string(), "ca-certificates".to_string()],
                env: BTreeMap::new(),
                copy: vec![CopySpec {
                    from: "target/release/app".to_string(),
                    to: "/usr/local/bin/app".to_string(),
                }],
                command: vec!["/usr/local/bin/app".to_string()],
                workdir: "/app".to_string(),
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

        // Verify it's valid YAML
        let _parsed: UniversalBuild = serde_yaml::from_str(&output).unwrap();
    }
}
