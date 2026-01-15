//! npm build system (JavaScript/TypeScript)

use super::node_common::{parse_node_version, read_node_version_file};
use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

pub struct NpmBuildSystem;

impl BuildSystem for NpmBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Npm
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "package.json".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "package-lock.json".to_string(),
                priority: 12,
            },
        ]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for rel_path in file_tree {
            let filename = rel_path.file_name().and_then(|n| n.to_str());

            let is_match = match filename {
                Some("package-lock.json") => true,
                Some("package.json") => {
                    let abs_path = repo_root.join(rel_path);
                    let content = fs.read_to_string(&abs_path).ok();
                    if let Some(c) = content.as_deref() {
                        !c.contains("\"packageManager\": \"pnpm")
                            && !c.contains("\"packageManager\": \"yarn")
                            && !c.contains("\"packageManager\": \"bun")
                    } else {
                        true
                    }
                }
                _ => false,
            };

            if is_match {
                detections.push(DetectionStack::new(
                    BuildSystemId::Npm,
                    LanguageId::JavaScript,
                    rel_path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let node_version = read_node_version_file(service_path)
            .or_else(|| manifest_content.and_then(parse_node_version))
            .or_else(|| wolfi_index.get_latest_version("nodejs"))
            .expect("Failed to get nodejs version from Wolfi index");

        let build_env = std::collections::HashMap::new();

        BuildTemplate {
            build_packages: vec![node_version, "npm".to_string()],
            build_commands: vec!["npm ci".to_string()],
            cache_paths: vec!["node_modules/".to_string(), "/root/.npm/".to_string()],
            common_ports: vec![3000, 8080],
            build_env,
            runtime_copy: vec![
                ("dist/".to_string(), "/app/dist/".to_string()),
                ("build/".to_string(), "/app/build/".to_string()),
            ],
            runtime_env: std::collections::HashMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".npm".to_string()]
    }
    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("\"workspaces\"")
        } else {
            false
        }
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![
            "lerna.json".to_string(),
            "nx.json".to_string(),
            "turbo.json".to_string(),
            "rush.json".to_string(),
        ]
    }

    fn parse_package_metadata(
        &self,
        manifest_content: &str,
    ) -> Result<(String, bool), anyhow::Error> {
        let package: serde_json::Value = serde_json::from_str(manifest_content)?;

        let name = package["name"].as_str().unwrap_or("unknown").to_string();

        let is_application = package["scripts"]["start"].is_string();

        Ok((name, is_application))
    }

    fn parse_workspace_patterns(
        &self,
        manifest_content: &str,
    ) -> Result<Vec<String>, anyhow::Error> {
        super::parse_package_json_workspaces(manifest_content)
    }

    fn glob_workspace_pattern(
        &self,
        repo_path: &std::path::Path,
        pattern: &str,
    ) -> Result<Vec<std::path::PathBuf>, anyhow::Error> {
        super::glob_package_json_workspace_pattern(repo_path, pattern)
    }
}
