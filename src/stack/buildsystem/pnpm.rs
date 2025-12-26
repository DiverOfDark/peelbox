//! pnpm build system (JavaScript/TypeScript)

use super::node_common::{parse_node_version, read_node_version_file};
use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct PnpmBuildSystem;

impl BuildSystem for PnpmBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Pnpm
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "pnpm-lock.yaml".to_string(),
                priority: 15,
            },
            ManifestPattern {
                filename: "package.json".to_string(),
                priority: 10,
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
                Some("pnpm-lock.yaml") => true,
                Some("package.json") => {
                    let abs_path = repo_root.join(rel_path);
                    let content = fs.read_to_string(&abs_path).ok();
                    if let Some(c) = content.as_deref() {
                        c.contains("\"packageManager\": \"pnpm")
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if is_match {
                detections.push(DetectionStack::new(
                    BuildSystemId::Pnpm,
                    LanguageId::JavaScript,
                    rel_path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let node_version = read_node_version_file(service_path)
            .or_else(|| manifest_content.and_then(|c| parse_node_version(c)))
            .or_else(|| wolfi_index.get_latest_version("nodejs"))
            .expect("Failed to get nodejs version from Wolfi index");

        BuildTemplate {
            build_packages: vec![node_version.clone()],
            build_commands: vec![
                "corepack enable".to_string(),
                "pnpm install --frozen-lockfile".to_string(),
                "pnpm build".to_string(),
            ],
            cache_paths: vec!["node_modules/".to_string(), ".pnpm-store/".to_string()],
            artifacts: vec!["dist/".to_string(), "build/".to_string()],
            common_ports: vec![3000, 8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".pnpm-store".to_string()]
    }
    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("\"workspaces\"")
        } else {
            false
        }
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec!["pnpm-workspace.yaml".to_string(), "turbo.json".to_string()]
    }

    fn parse_workspace_patterns(&self, manifest_content: &str) -> Result<Vec<String>> {
        super::parse_package_json_workspaces(manifest_content)
    }

    fn glob_workspace_pattern(
        &self,
        repo_path: &std::path::Path,
        pattern: &str,
    ) -> Result<Vec<std::path::PathBuf>> {
        super::glob_package_json_workspace_pattern(repo_path, pattern)
    }
}
