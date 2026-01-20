use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub endpoint: String,
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

fn deserialize_null_default_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_else(default_version))
}

fn default_version() -> String {
    "1.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalBuild {
    #[serde(
        default = "default_version",
        deserialize_with = "deserialize_null_default_version"
    )]
    pub version: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub metadata: BuildMetadata,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub build: BuildStage,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub runtime: RuntimeStage,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub language: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub build_system: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildStage {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub packages: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub env: HashMap<String, String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub commands: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub cache: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeStage {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub packages: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub env: HashMap<String, String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub copy: Vec<CopySpec>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub command: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub workdir: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub ports: Vec<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CopySpec {
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub from: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub to: String,
}

impl fmt::Display for UniversalBuild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_yaml() {
            Ok(yaml) => write!(f, "{}", yaml),
            Err(e) => write!(f, "Error formatting UniversalBuild: {}", e),
        }
    }
}

impl UniversalBuild {
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("Failed to serialize UniversalBuild to YAML")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_minimal_valid_build() -> UniversalBuild {
        UniversalBuild {
            version: "1.0".to_string(),
            metadata: BuildMetadata {
                project_name: Some("test-app".to_string()),
                language: "rust".to_string(),
                build_system: "cargo".to_string(),
                framework: None,
                reasoning: "Detected Cargo.toml".to_string(),
            },
            build: BuildStage {
                packages: vec!["rust".to_string(), "build-base".to_string()],
                env: HashMap::new(),
                commands: vec!["cargo build --release".to_string()],
                cache: vec![],
            },
            runtime: RuntimeStage {
                packages: vec!["glibc".to_string(), "ca-certificates".to_string()],
                env: HashMap::new(),
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
    fn test_valid_build() {
        let _build = create_minimal_valid_build();
    }

    #[test]
    fn test_to_yaml() {
        let build = create_minimal_valid_build();
        let yaml = build.to_yaml();
        assert!(yaml.is_ok());
        let yaml_str = yaml.unwrap();
        assert!(yaml_str.contains("version:"));
        assert!(yaml_str.contains("metadata:"));
        assert!(yaml_str.contains("build:"));
        assert!(yaml_str.contains("runtime:"));
    }

    #[test]
    fn test_serialization_deserialization() {
        let build = create_minimal_valid_build();
        let yaml = build.to_yaml().unwrap();
        let deserialized: UniversalBuild = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(build.version, deserialized.version);
        assert_eq!(build.metadata.language, deserialized.metadata.language);
        assert_eq!(build.build.packages, deserialized.build.packages);
        assert_eq!(build.runtime.packages, deserialized.runtime.packages);
    }

    #[test]
    fn test_display_yaml_format() {
        let build = create_minimal_valid_build();
        let display = format!("{}", build);

        // YAML output should contain all major sections
        assert!(display.contains("version:"));
        assert!(display.contains("metadata:"));
        assert!(display.contains("language: rust"));
        assert!(display.contains("build_system: cargo"));
        assert!(display.contains("build:"));
        assert!(display.contains("packages:"));
        assert!(display.contains("cargo build --release"));
        assert!(display.contains("runtime:"));
        assert!(display.contains("reasoning: Detected Cargo.toml"));
    }

    #[test]
    fn test_display_shows_all_fields() {
        let mut build = create_minimal_valid_build();
        build.build.packages = vec!["pkg-config".to_string(), "libssl-dev".to_string()];
        build
            .build
            .env
            .insert("CARGO_HOME".to_string(), "/cache/cargo".to_string());
        build.build.cache = vec!["/cache/cargo".to_string()];
        build.runtime.packages = vec!["ca-certificates".to_string()];
        build
            .runtime
            .env
            .insert("PORT".to_string(), "8080".to_string());
        build.runtime.ports = vec![8080, 8443];

        let display = format!("{}", build);

        // Verify all fields are present in YAML output
        assert!(display.contains("packages:"));
        assert!(display.contains("pkg-config"));
        assert!(display.contains("libssl-dev"));
        assert!(display.contains("env:"));
        assert!(display.contains("CARGO_HOME"));
        assert!(display.contains("cache:"));
        assert!(display.contains("ports:"));
        assert!(display.contains("8080"));
    }

    #[test]
    fn test_display_with_copy_specs() {
        let build = create_minimal_valid_build();
        let display = format!("{}", build);

        // Verify copy specifications are shown
        assert!(display.contains("copy:"));
        assert!(display.contains("from: target/release/app"));
        assert!(display.contains("to: /usr/local/bin/app"));
    }

    #[test]
    fn test_deserialize_minimal_universal_build() {
        let minimal_json = r#"{
            "metadata": {},
            "build": {},
            "runtime": {}
        }"#;

        let result: Result<UniversalBuild, _> = serde_json::from_str(minimal_json);
        assert!(result.is_ok());

        let build = result.unwrap();
        assert_eq!(build.version, "1.0");
        assert_eq!(build.metadata.language, "");
        assert_eq!(build.metadata.build_system, "");
        assert_eq!(build.metadata.reasoning, "");
        assert!(build.build.commands.is_empty());
        assert!(build.runtime.copy.is_empty());
        assert!(build.runtime.command.is_empty());
    }

    #[test]
    fn test_deserialize_with_null_values() {
        let json_with_nulls = r#"{
            "version": null,
            "metadata": {
                "language": null,
                "build_system": null,
                "reasoning": null
            },
            "build": {
                "packages": null,
                "commands": null,
                "context": null,
                "artifacts": null
            },
            "runtime": {
                "packages": null,
                "copy": null,
                "command": null
            }
        }"#;

        let result: Result<UniversalBuild, _> = serde_json::from_str(json_with_nulls);
        assert!(result.is_ok());

        let build = result.unwrap();
        assert_eq!(build.version, "1.0");
        assert_eq!(build.metadata.language, "");
        assert!(build.build.packages.is_empty());
        assert!(build.build.commands.is_empty());
        assert!(build.runtime.copy.is_empty());
    }

    #[test]
    fn test_deserialize_missing_optional_fields() {
        let json = r#"{
            "metadata": {
                "language": "rust",
                "build_system": "cargo"
            },
            "build": {
                "packages": ["rust", "build-base"],
                "commands": ["cargo build --release"]
            },
            "runtime": {
                "packages": ["glibc"],
                "command": ["./app"]
            }
        }"#;

        let result: Result<UniversalBuild, _> = serde_json::from_str(json);
        assert!(result.is_ok());

        let build = result.unwrap();
        assert_eq!(build.version, "1.0");
        assert_eq!(build.metadata.project_name, None);
        assert_eq!(build.metadata.reasoning, "");
        assert_eq!(build.build.packages, vec!["rust", "build-base"]);
        assert!(build.build.env.is_empty());
        assert!(build.build.cache.is_empty());
        assert_eq!(build.runtime.packages, vec!["glibc"]);
        assert!(build.runtime.ports.is_empty());
    }

    #[test]
    fn test_deserialize_empty_copy_spec() {
        let json = r#"{
            "metadata": {},
            "build": {},
            "runtime": {
                "copy": [{}]
            }
        }"#;

        let result: Result<UniversalBuild, _> = serde_json::from_str(json);
        assert!(result.is_ok());

        let build = result.unwrap();
        assert_eq!(build.runtime.copy.len(), 1);
        assert_eq!(build.runtime.copy[0].from, "");
        assert_eq!(build.runtime.copy[0].to, "");
    }

    #[test]
    fn test_validation_still_works_after_defaults() {
        let _minimal_build = UniversalBuild {
            version: "".to_string(),
            metadata: BuildMetadata {
                project_name: None,
                language: "".to_string(),
                build_system: "".to_string(),
                framework: None,
                reasoning: "".to_string(),
            },
            build: BuildStage {
                packages: vec![],
                env: HashMap::new(),
                commands: vec![],
                cache: vec![],
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: HashMap::new(),
                copy: vec![],
                command: vec![],
                workdir: String::new(),
                ports: vec![],
                health: None,
            },
        };
    }

    #[test]
    fn test_deserialize_partial_valid_build() {
        let json = r#"{
            "version": "1.0",
            "metadata": {
                "language": "rust",
                "build_system": "cargo"
            },
            "build": {
                "packages": ["rust", "build-base"],
                "commands": ["cargo build --release"]
            },
            "runtime": {
                "packages": ["glibc", "ca-certificates"],
                "copy": [
                    {
                        "from": "target/release/app",
                        "to": "/usr/local/bin/app"
                    }
                ],
                "command": ["/usr/local/bin/app"]
            }
        }"#;

        let result: Result<UniversalBuild, _> = serde_json::from_str(json);
        if let Err(e) = &result {
            eprintln!("Deserialization error: {}", e);
        }
        assert!(result.is_ok());

        let build = result.unwrap();
        assert_eq!(build.metadata.reasoning, "");
        assert_eq!(build.build.packages, vec!["rust", "build-base"]);
        assert!(build.runtime.ports.is_empty());
    }
}
