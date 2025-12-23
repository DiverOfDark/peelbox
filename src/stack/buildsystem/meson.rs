//! Meson build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct MesonBuildSystem;

impl BuildSystem for MesonBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Meson
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "meson.build".to_string(),
            priority: 9,
        }]
    }

    fn detect_all(
        &self,
        _repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for rel_path in file_tree {
            if rel_path.file_name().and_then(|n| n.to_str()) == Some("meson.build") {
                let abs_path = rel_path.clone();
                let content = fs.read_to_string(&abs_path).ok();

                let is_valid = if let Some(c) = content.as_deref() {
                    c.contains("project(")
                } else {
                    true
                };

                if is_valid {
                    detections.push(DetectionStack::new(
                        BuildSystemId::Meson,
                        LanguageId::Cpp,
                        rel_path.clone(),
                    ));
                }
            }
        }

        Ok(detections)
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "gcc:latest".to_string(),
            runtime_image: "alpine:3.19".to_string(),
            build_packages: vec!["meson".to_string(), "ninja-build".to_string()],
            runtime_packages: vec![],
            build_commands: vec![
                "meson setup builddir".to_string(),
                "meson compile -C builddir".to_string(),
            ],
            cache_paths: vec![],
            artifacts: vec!["builddir/app".to_string()],
            common_ports: vec![],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["builddir".to_string()]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }
}
