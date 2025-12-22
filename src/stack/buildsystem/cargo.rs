//! Cargo build system (Rust)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use anyhow::Result;
use toml::Value;

pub struct CargoBuildSystem;

impl BuildSystem for CargoBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Cargo
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "Cargo.toml".to_string(),
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "Cargo.toml" {
            return false;
        }

        if let Some(content) = manifest_content {
            content.contains("[package]") || content.contains("[workspace]")
        } else {
            true
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "rust:1.75".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            build_packages: vec!["pkg-config".to_string(), "libssl-dev".to_string()],
            runtime_packages: vec!["ca-certificates".to_string(), "libssl3".to_string()],
            build_commands: vec!["cargo build --release".to_string()],
            cache_paths: vec![
                "target/".to_string(),
                "/usr/local/cargo/registry/".to_string(),
            ],
            artifacts: vec!["target/release/{project_name}".to_string()],
            common_ports: vec![8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["target".to_string(), ".cargo".to_string()]
    }

    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("[workspace]")
        } else {
            false
        }
    }

    fn parse_workspace_patterns(&self, manifest_content: &str) -> Result<Vec<String>> {
        let value: Value = toml::from_str(manifest_content)?;

        if let Some(members) = value
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
        {
            Ok(members
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect())
        } else {
            Ok(vec![])
        }
    }
}
