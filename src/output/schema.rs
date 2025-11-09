//! UniversalBuild schema data structures
//!
//! This module defines the schema for the UniversalBuild format - a declarative
//! container build specification that LLMs can generate to describe how to build
//! and package applications for container deployment.

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;

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

/// Main UniversalBuild structure representing a complete container build specification
///
/// This is the root structure that LLMs will generate to describe how to build
/// and run an application in a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalBuild {
    /// Schema version (e.g., "1.0")
    #[serde(default = "default_version", deserialize_with = "deserialize_null_default_version")]
    pub version: String,
    /// Project metadata and detection information
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub metadata: BuildMetadata,
    /// Build stage configuration
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub build: BuildStage,
    /// Runtime stage configuration
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub runtime: RuntimeStage,
}

/// Metadata about the detected project and build system
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildMetadata {
    /// Optional project name (if detected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    /// Detected programming language (e.g., "rust", "nodejs", "python")
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub language: String,
    /// Detected build system (e.g., "cargo", "npm", "maven", "gradle")
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub build_system: String,
    /// Confidence score from 0.0 to 1.0
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub confidence: f32,
    /// Human-readable explanation of the detection reasoning
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub reasoning: String,
}

/// Build stage configuration - defines how to compile/build the application
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildStage {
    /// Base Docker image for the build stage (e.g., "rust:1.75", "node:20")
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub base: String,
    /// System packages to install (e.g., ["build-essential", "pkg-config"])
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub packages: Vec<String>,
    /// Environment variables for the build stage
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub env: HashMap<String, String>,
    /// Build commands to execute in order
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub commands: Vec<String>,
    /// Files/directories to copy from source (pairs: [source, destination])
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub context: Vec<String>,
    /// Directories to cache between builds (e.g., ["/usr/local/cargo/registry"])
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub cache: Vec<String>,
    /// Build artifacts to preserve (e.g., ["target/release/myapp"])
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub artifacts: Vec<String>,
}

/// Runtime stage configuration - defines the final container environment
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeStage {
    /// Base Docker image for runtime (e.g., "debian:bookworm-slim", "alpine:3.19")
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub base: String,
    /// Runtime system packages to install
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub packages: Vec<String>,
    /// Runtime environment variables
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub env: HashMap<String, String>,
    /// Files to copy from build stage
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub copy: Vec<CopySpec>,
    /// Container entrypoint command
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub command: Vec<String>,
    /// Ports to expose
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub ports: Vec<u16>,
    /// Optional health check configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<Healthcheck>,
}

/// Specification for copying files from build stage to runtime stage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CopySpec {
    /// Source path in build stage
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub from: String,
    /// Destination path in runtime stage
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub to: String,
}

/// Container health check configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Healthcheck {
    /// Health check command (e.g., ["CMD", "curl", "-f", "http://localhost/health"])
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub test: Vec<String>,
    /// Interval between health checks (e.g., "30s")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    /// Timeout for each health check (e.g., "3s")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<String>,
    /// Number of consecutive failures before marking unhealthy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<u32>,
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
    /// Serialize the UniversalBuild to YAML format
    ///
    /// # Returns
    /// YAML string representation of the build specification
    ///
    /// # Errors
    /// Returns error if serialization fails
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("Failed to serialize UniversalBuild to YAML")
    }

    /// Validate the UniversalBuild structure
    ///
    /// Checks:
    /// - Version format
    /// - Confidence score range (0.0-1.0)
    /// - Required fields are non-empty
    /// - Context has even number of elements (source/dest pairs)
    ///
    /// # Returns
    /// Ok(()) if valid, Err otherwise
    pub fn validate(&self) -> Result<()> {
        // Validate version format
        if self.version.is_empty() {
            anyhow::bail!("Version cannot be empty");
        }

        // Validate confidence score
        if !(0.0..=1.0).contains(&self.metadata.confidence) {
            anyhow::bail!(
                "Confidence score must be between 0.0 and 1.0, got {}",
                self.metadata.confidence
            );
        }

        // Validate required metadata fields
        if self.metadata.language.is_empty() {
            anyhow::bail!("Language cannot be empty");
        }
        if self.metadata.build_system.is_empty() {
            anyhow::bail!("Build system cannot be empty");
        }

        // Validate build stage
        if self.build.base.is_empty() {
            anyhow::bail!("Build base image cannot be empty");
        }
        if self.build.commands.is_empty() {
            anyhow::bail!("Build commands cannot be empty");
        }
        if self.build.context.is_empty() {
            anyhow::bail!("Build context cannot be empty");
        }
        // Context should have pairs of (source, destination)
        if self.build.context.len() % 2 != 0 {
            anyhow::bail!(
                "Build context must have an even number of elements (source/dest pairs), got {}",
                self.build.context.len()
            );
        }
        if self.build.artifacts.is_empty() {
            anyhow::bail!("Build artifacts cannot be empty");
        }

        // Validate runtime stage
        if self.runtime.base.is_empty() {
            anyhow::bail!("Runtime base image cannot be empty");
        }
        if self.runtime.copy.is_empty() {
            anyhow::bail!("Runtime copy specifications cannot be empty");
        }
        for (i, copy_spec) in self.runtime.copy.iter().enumerate() {
            if copy_spec.from.is_empty() {
                anyhow::bail!("Runtime copy[{}] 'from' path cannot be empty", i);
            }
            if copy_spec.to.is_empty() {
                anyhow::bail!("Runtime copy[{}] 'to' path cannot be empty", i);
            }
        }
        if self.runtime.command.is_empty() {
            anyhow::bail!("Runtime command cannot be empty");
        }

        Ok(())
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
                confidence: 0.95,
                reasoning: "Detected Cargo.toml".to_string(),
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
    fn test_valid_build() {
        let build = create_minimal_valid_build();
        assert!(build.validate().is_ok());
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
    fn test_invalid_confidence() {
        let mut build = create_minimal_valid_build();
        build.metadata.confidence = 1.5;
        assert!(build.validate().is_err());
    }

    #[test]
    fn test_empty_version() {
        let mut build = create_minimal_valid_build();
        build.version = "".to_string();
        assert!(build.validate().is_err());
    }

    #[test]
    fn test_empty_language() {
        let mut build = create_minimal_valid_build();
        build.metadata.language = "".to_string();
        assert!(build.validate().is_err());
    }

    #[test]
    fn test_empty_build_commands() {
        let mut build = create_minimal_valid_build();
        build.build.commands = vec![];
        assert!(build.validate().is_err());
    }

    #[test]
    fn test_invalid_context_pairs() {
        let mut build = create_minimal_valid_build();
        build.build.context = vec![".".to_string()]; // Odd number
        assert!(build.validate().is_err());
    }

    #[test]
    fn test_empty_runtime_copy() {
        let mut build = create_minimal_valid_build();
        build.runtime.copy = vec![];
        assert!(build.validate().is_err());
    }

    #[test]
    fn test_serialization_deserialization() {
        let build = create_minimal_valid_build();
        let yaml = build.to_yaml().unwrap();
        let deserialized: UniversalBuild = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(build.version, deserialized.version);
        assert_eq!(build.metadata.language, deserialized.metadata.language);
        assert_eq!(build.build.base, deserialized.build.base);
        assert_eq!(build.runtime.base, deserialized.runtime.base);
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
        assert!(display.contains("confidence: 0.95"));
        assert!(display.contains("build:"));
        assert!(display.contains("base: rust:1.75"));
        assert!(display.contains("cargo build --release"));
        assert!(display.contains("runtime:"));
        assert!(display.contains("base: debian:bookworm-slim"));
        assert!(display.contains("reasoning: Detected Cargo.toml"));
    }

    #[test]
    fn test_display_shows_all_fields() {
        let mut build = create_minimal_valid_build();
        build.build.packages = vec!["pkg-config".to_string(), "libssl-dev".to_string()];
        build.build.env.insert("CARGO_HOME".to_string(), "/cache/cargo".to_string());
        build.build.cache = vec!["/cache/cargo".to_string()];
        build.runtime.packages = vec!["ca-certificates".to_string()];
        build.runtime.env.insert("PORT".to_string(), "8080".to_string());
        build.runtime.ports = vec![8080, 8443];
        build.runtime.healthcheck = Some(Healthcheck {
            test: vec!["CMD".to_string(), "curl".to_string(), "-f".to_string(), "http://localhost/health".to_string()],
            interval: Some("30s".to_string()),
            timeout: Some("3s".to_string()),
            retries: Some(3),
        });

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
        assert!(display.contains("healthcheck:"));
        assert!(display.contains("interval:"));
        assert!(display.contains("timeout:"));
        assert!(display.contains("retries:"));
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
        assert_eq!(build.metadata.confidence, 0.0);
        assert_eq!(build.metadata.reasoning, "");
        assert!(build.build.commands.is_empty());
        assert!(build.build.context.is_empty());
        assert!(build.build.artifacts.is_empty());
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
                "confidence": null,
                "reasoning": null
            },
            "build": {
                "base": null,
                "commands": null,
                "context": null,
                "artifacts": null
            },
            "runtime": {
                "base": null,
                "copy": null,
                "command": null
            }
        }"#;

        let result: Result<UniversalBuild, _> = serde_json::from_str(json_with_nulls);
        assert!(result.is_ok());

        let build = result.unwrap();
        assert_eq!(build.version, "1.0");
        assert_eq!(build.metadata.language, "");
        assert_eq!(build.build.base, "");
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
                "base": "rust:1.75",
                "commands": ["cargo build --release"]
            },
            "runtime": {
                "base": "debian:bookworm-slim",
                "command": ["./app"]
            }
        }"#;

        let result: Result<UniversalBuild, _> = serde_json::from_str(json);
        assert!(result.is_ok());

        let build = result.unwrap();
        assert_eq!(build.version, "1.0");
        assert_eq!(build.metadata.project_name, None);
        assert_eq!(build.metadata.confidence, 0.0);
        assert_eq!(build.metadata.reasoning, "");
        assert!(build.build.packages.is_empty());
        assert!(build.build.env.is_empty());
        assert!(build.build.cache.is_empty());
        assert!(build.runtime.packages.is_empty());
        assert!(build.runtime.ports.is_empty());
        assert_eq!(build.runtime.healthcheck, None);
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
    fn test_deserialize_empty_healthcheck() {
        let json = r#"{
            "metadata": {},
            "build": {},
            "runtime": {
                "healthcheck": {}
            }
        }"#;

        let result: Result<UniversalBuild, _> = serde_json::from_str(json);
        assert!(result.is_ok());

        let build = result.unwrap();
        assert!(build.runtime.healthcheck.is_some());
        let healthcheck = build.runtime.healthcheck.unwrap();
        assert!(healthcheck.test.is_empty());
        assert_eq!(healthcheck.interval, None);
        assert_eq!(healthcheck.timeout, None);
        assert_eq!(healthcheck.retries, None);
    }

    #[test]
    fn test_validation_still_works_after_defaults() {
        let minimal_build = UniversalBuild {
            version: "".to_string(),
            metadata: BuildMetadata {
                project_name: None,
                language: "".to_string(),
                build_system: "".to_string(),
                confidence: 0.0,
                reasoning: "".to_string(),
            },
            build: BuildStage {
                base: "".to_string(),
                packages: vec![],
                env: HashMap::new(),
                commands: vec![],
                context: vec![],
                cache: vec![],
                artifacts: vec![],
            },
            runtime: RuntimeStage {
                base: "".to_string(),
                packages: vec![],
                env: HashMap::new(),
                copy: vec![],
                command: vec![],
                ports: vec![],
                healthcheck: None,
            },
        };

        let validation_result = minimal_build.validate();
        assert!(validation_result.is_err());
    }

    #[test]
    fn test_deserialize_partial_valid_build() {
        let json = r#"{
            "version": "1.0",
            "metadata": {
                "language": "rust",
                "build_system": "cargo",
                "confidence": 0.95
            },
            "build": {
                "base": "rust:1.75",
                "commands": ["cargo build --release"],
                "context": [".", "/app"],
                "artifacts": ["target/release/app"]
            },
            "runtime": {
                "base": "debian:bookworm-slim",
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
        assert!(result.is_ok());

        let build = result.unwrap();
        assert!(build.validate().is_ok());
        assert_eq!(build.metadata.reasoning, "");
        assert!(build.build.packages.is_empty());
        assert!(build.runtime.ports.is_empty());
    }
}
