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

    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let mut build_packages = vec!["build-base".to_string()];

        if wolfi_index.has_package("meson") {
            build_packages.push("meson".to_string());
        }
        if wolfi_index.has_package("ninja") {
            build_packages.push("ninja".to_string());
        }

        BuildTemplate {
            build_packages,
            build_commands: vec![
                "meson setup builddir".to_string(),
                "meson compile -C builddir".to_string(),
            ],
            cache_paths: vec![],

            common_ports: vec![],
            build_env: std::collections::HashMap::new(),
            runtime_copy: vec![("builddir/app".to_string(), "/usr/local/bin/app".to_string())],
            runtime_env: std::collections::HashMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["builddir".to_string()]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }
}
