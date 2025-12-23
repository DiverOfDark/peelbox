//! Bun build system (JavaScript/TypeScript)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct BunBuildSystem;

impl BuildSystem for BunBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Bun
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "bun.lockb".to_string(),
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
                Some("bun.lockb") => true,
                Some("package.json") => {
                    let abs_path = repo_root.join(rel_path);
                    let content = fs.read_to_string(&abs_path).ok();
                    if let Some(c) = content.as_deref() {
                        c.contains("\"packageManager\": \"bun")
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if is_match {
                detections.push(DetectionStack::new(
                    BuildSystemId::Bun,
                    LanguageId::JavaScript,
                    rel_path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "oven/bun:1".to_string(),
            runtime_image: "oven/bun:1-slim".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec!["bun install".to_string(), "bun run build".to_string()],
            cache_paths: vec!["node_modules/".to_string(), ".bun/".to_string()],
            artifacts: vec!["dist/".to_string(), "build/".to_string()],
            common_ports: vec![3000, 8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".bun".to_string()]
    }

    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("\"workspaces\"")
        } else {
            false
        }
    }
}
