//! Cargo build system (Rust)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};
use toml::Value;

pub struct CargoBuildSystem;

impl BuildSystem for CargoBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Cargo
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "Cargo.toml".to_string(),
            priority: 10,
        }]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for rel_path in file_tree {
            if rel_path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml") {
                let abs_path = repo_root.join(rel_path);
                let content = fs.read_to_string(&abs_path).ok();

                let is_valid = if let Some(c) = content.as_deref() {
                    c.contains("[package]") || c.contains("[workspace]")
                } else {
                    true
                };

                if is_valid {
                    detections.push(DetectionStack::new(
                        BuildSystemId::Cargo,
                        LanguageId::Rust,
                        rel_path.clone(),
                    ));
                }
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let mut build_packages = Vec::new();

        // Wolfi uses versioned rust packages (rust-1.92, rust-1.91, etc.)
        if let Some(rust_package) = wolfi_index.get_latest_version("rust") {
            build_packages.push(rust_package);
        }

        build_packages.push("build-base".to_string());

        BuildTemplate {
            build_packages,
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
