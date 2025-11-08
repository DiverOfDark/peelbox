//! UniversalBuild schema data structures
//!
//! This module defines the schema for the UniversalBuild format - a declarative
//! container build specification that LLMs can generate to describe how to build
//! and package applications for container deployment.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Main UniversalBuild structure representing a complete container build specification
///
/// This is the root structure that LLMs will generate to describe how to build
/// and run an application in a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalBuild {
    /// Schema version (e.g., "1.0")
    pub version: String,
    /// Project metadata and detection information
    pub metadata: BuildMetadata,
    /// Build stage configuration
    pub build: BuildStage,
    /// Runtime stage configuration
    pub runtime: RuntimeStage,
}

/// Metadata about the detected project and build system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildMetadata {
    /// Optional project name (if detected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    /// Detected programming language (e.g., "rust", "nodejs", "python")
    pub language: String,
    /// Detected build system (e.g., "cargo", "npm", "maven", "gradle")
    pub build_system: String,
    /// Confidence score from 0.0 to 1.0
    pub confidence: f32,
    /// Human-readable explanation of the detection reasoning
    pub reasoning: String,
}

/// Build stage configuration - defines how to compile/build the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStage {
    /// Base Docker image for the build stage (e.g., "rust:1.75", "node:20")
    pub base: String,
    /// System packages to install (e.g., ["build-essential", "pkg-config"])
    #[serde(default)]
    pub packages: Vec<String>,
    /// Environment variables for the build stage
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Build commands to execute in order
    pub commands: Vec<String>,
    /// Files/directories to copy from source (pairs: [source, destination])
    pub context: Vec<String>,
    /// Directories to cache between builds (e.g., ["/usr/local/cargo/registry"])
    #[serde(default)]
    pub cache: Vec<String>,
    /// Build artifacts to preserve (e.g., ["target/release/myapp"])
    pub artifacts: Vec<String>,
}

/// Runtime stage configuration - defines the final container environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStage {
    /// Base Docker image for runtime (e.g., "debian:bookworm-slim", "alpine:3.19")
    pub base: String,
    /// Runtime system packages to install
    #[serde(default)]
    pub packages: Vec<String>,
    /// Runtime environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Files to copy from build stage
    pub copy: Vec<CopySpec>,
    /// Container entrypoint command
    pub command: Vec<String>,
    /// Ports to expose
    #[serde(default)]
    pub ports: Vec<u16>,
    /// Optional health check configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<Healthcheck>,
}

/// Specification for copying files from build stage to runtime stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopySpec {
    /// Source path in build stage
    pub from: String,
    /// Destination path in runtime stage
    pub to: String,
}

/// Container health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Healthcheck {
    /// Health check command (e.g., ["CMD", "curl", "-f", "http://localhost/health"])
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

fn confidence_level(confidence: f32) -> &'static str {
    match confidence {
        c if c >= 0.9 => "Very High",
        c if c >= 0.8 => "High",
        c if c >= 0.7 => "Moderate",
        c if c >= 0.6 => "Low",
        _ => "Very Low",
    }
}

fn format_ports(ports: &[u16]) -> String {
    if ports.len() > 5 {
        format!(
            "{} (and {} more)",
            ports[..5]
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            ports.len() - 5
        )
    } else {
        ports
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for UniversalBuild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "UniversalBuild Detection Result")?;
        writeln!(f, "================================")?;
        writeln!(f, "Language: {}", self.metadata.language)?;
        writeln!(f, "Build System: {}", self.metadata.build_system)?;
        if let Some(ref project_name) = self.metadata.project_name {
            writeln!(f, "Project: {}", project_name)?;
        }
        writeln!(
            f,
            "Confidence: {:.1}% ({})",
            self.metadata.confidence * 100.0,
            confidence_level(self.metadata.confidence)
        )?;
        writeln!(f)?;

        writeln!(f, "Build Stage:")?;
        writeln!(f, "  Base Image: {}", self.build.base)?;
        if !self.build.packages.is_empty() {
            let pkg_display = if self.build.packages.len() > 3 {
                format!(
                    "{} (and {} more)",
                    self.build.packages[..3].join(", "),
                    self.build.packages.len() - 3
                )
            } else {
                self.build.packages.join(", ")
            };
            writeln!(f, "  Packages:   {}", pkg_display)?;
        }
        writeln!(f, "  Commands:")?;
        for cmd in &self.build.commands {
            writeln!(f, "    - {}", cmd)?;
        }
        if !self.build.artifacts.is_empty() {
            writeln!(f, "  Artifacts:")?;
            for artifact in &self.build.artifacts {
                writeln!(f, "    - {}", artifact)?;
            }
        }
        writeln!(f)?;

        writeln!(f, "Runtime Stage:")?;
        writeln!(f, "  Base Image: {}", self.runtime.base)?;
        if !self.runtime.packages.is_empty() {
            let pkg_display = if self.runtime.packages.len() > 3 {
                format!(
                    "{} (and {} more)",
                    self.runtime.packages[..3].join(", "),
                    self.runtime.packages.len() - 3
                )
            } else {
                self.runtime.packages.join(", ")
            };
            writeln!(f, "  Packages:   {}", pkg_display)?;
        }
        writeln!(f, "  Command:    {}", self.runtime.command.join(" "))?;
        if !self.runtime.ports.is_empty() {
            writeln!(f, "  Ports:      {}", format_ports(&self.runtime.ports))?;
        }
        if let Some(ref healthcheck) = self.runtime.healthcheck {
            writeln!(f, "  Healthcheck: {}", healthcheck.test.join(" "))?;
        }
        writeln!(f)?;

        writeln!(f, "Reasoning:")?;
        writeln!(f, "  {}", self.metadata.reasoning)?;

        Ok(())
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
    fn test_display_minimal() {
        let build = create_minimal_valid_build();
        let display = format!("{}", build);

        assert!(display.contains("UniversalBuild Detection Result"));
        assert!(display.contains("Language: rust"));
        assert!(display.contains("Build System: cargo"));
        assert!(display.contains("95.0%"));
        assert!(display.contains("Very High"));
        assert!(display.contains("Build Stage:"));
        assert!(display.contains("Base Image: rust:1.75"));
        assert!(display.contains("cargo build --release"));
        assert!(display.contains("Runtime Stage:"));
        assert!(display.contains("Base Image: debian:bookworm-slim"));
        assert!(display.contains("Reasoning:"));
        assert!(display.contains("Detected Cargo.toml"));
    }

    #[test]
    fn test_display_with_packages() {
        let mut build = create_minimal_valid_build();
        build.build.packages = vec!["pkg-config".to_string(), "libssl-dev".to_string()];
        build.runtime.packages = vec!["ca-certificates".to_string(), "libssl3".to_string()];

        let display = format!("{}", build);
        assert!(display.contains("Packages:   pkg-config, libssl-dev"));
        assert!(display.contains("Packages:   ca-certificates, libssl3"));
    }

    #[test]
    fn test_display_truncates_long_package_lists() {
        let mut build = create_minimal_valid_build();
        build.build.packages = vec![
            "pkg1".to_string(),
            "pkg2".to_string(),
            "pkg3".to_string(),
            "pkg4".to_string(),
            "pkg5".to_string(),
        ];

        let display = format!("{}", build);
        assert!(display.contains("pkg1, pkg2, pkg3"));
        assert!(display.contains("and 2 more"));
    }

    #[test]
    fn test_display_with_ports() {
        let mut build = create_minimal_valid_build();
        build.runtime.ports = vec![8080, 8443, 9000];

        let display = format!("{}", build);
        assert!(display.contains("Ports:      8080, 8443, 9000"));
    }

    #[test]
    fn test_display_truncates_many_ports() {
        let mut build = create_minimal_valid_build();
        build.runtime.ports = vec![8080, 8443, 9000, 9090, 3000, 5000, 6000];

        let display = format!("{}", build);
        assert!(display.contains("8080, 8443, 9000, 9090, 3000"));
        assert!(display.contains("and 2 more"));
    }

    #[test]
    fn test_display_with_healthcheck() {
        let mut build = create_minimal_valid_build();
        build.runtime.healthcheck = Some(Healthcheck {
            test: vec![
                "CMD".to_string(),
                "curl".to_string(),
                "-f".to_string(),
                "http://localhost/health".to_string(),
            ],
            interval: Some("30s".to_string()),
            timeout: Some("3s".to_string()),
            retries: Some(3),
        });

        let display = format!("{}", build);
        assert!(display.contains("Healthcheck: CMD curl -f http://localhost/health"));
    }

    #[test]
    fn test_display_with_project_name() {
        let mut build = create_minimal_valid_build();
        build.metadata.project_name = Some("my-awesome-app".to_string());

        let display = format!("{}", build);
        assert!(display.contains("Project: my-awesome-app"));
    }

    #[test]
    fn test_confidence_level_function() {
        use super::confidence_level;

        assert_eq!(confidence_level(0.95), "Very High");
        assert_eq!(confidence_level(0.85), "High");
        assert_eq!(confidence_level(0.75), "Moderate");
        assert_eq!(confidence_level(0.65), "Low");
        assert_eq!(confidence_level(0.45), "Very Low");
    }

    #[test]
    fn test_format_ports_function() {
        use super::format_ports;

        assert_eq!(format_ports(&[8080]), "8080");
        assert_eq!(format_ports(&[8080, 8443]), "8080, 8443");
        assert_eq!(
            format_ports(&[8080, 8443, 9000, 9090, 3000]),
            "8080, 8443, 9000, 9090, 3000"
        );
        assert_eq!(
            format_ports(&[8080, 8443, 9000, 9090, 3000, 5000]),
            "8080, 8443, 9000, 9090, 3000 (and 1 more)"
        );
        assert_eq!(
            format_ports(&[8080, 8443, 9000, 9090, 3000, 5000, 6000, 7000]),
            "8080, 8443, 9000, 9090, 3000 (and 3 more)"
        );
    }
}
