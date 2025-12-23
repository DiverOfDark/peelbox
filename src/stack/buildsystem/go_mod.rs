//! Go modules build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct GoModBuildSystem;

impl BuildSystem for GoModBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::GoMod
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "go.mod".to_string(),
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
            if rel_path.file_name().and_then(|n| n.to_str()) == Some("go.mod") {
                let abs_path = repo_root.join(rel_path);
                let content = fs.read_to_string(&abs_path).ok();

                let is_valid = if let Some(c) = content.as_deref() {
                    c.contains("module ")
                } else {
                    true
                };

                if is_valid {
                    detections.push(DetectionStack::new(
                        BuildSystemId::GoMod,
                        LanguageId::Go,
                        rel_path.clone(),
                    ));
                }
            }
        }

        Ok(detections)
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "golang:1.21".to_string(),
            runtime_image: "alpine:3.19".to_string(),
            build_packages: vec![],
            runtime_packages: vec!["ca-certificates".to_string()],
            build_commands: vec!["go build -o app .".to_string()],
            cache_paths: vec![
                "/go/pkg/mod/".to_string(),
                "/root/.cache/go-build/".to_string(),
            ],
            artifacts: vec!["app".to_string()],
            common_ports: vec![8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".cache/go-build".to_string()]
    }
    fn workspace_configs(&self) -> Vec<String> {
        vec!["go.work".to_string()]
    }
}
