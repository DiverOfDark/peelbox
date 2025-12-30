//! Make build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct MakeBuildSystem;

impl BuildSystem for MakeBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Make
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "Makefile".to_string(),
            priority: 8,
        }]
    }

    fn detect_all(
        &self,
        _repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for path in file_tree {
            let filename = path.file_name().and_then(|n| n.to_str());
            if filename == Some("Makefile") || filename == Some("makefile") {
                detections.push(DetectionStack::new(
                    BuildSystemId::Make,
                    LanguageId::Cpp,
                    path.clone(),
                ));
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

        if wolfi_index.has_package("make") {
            build_packages.push("make".to_string());
        }
        if wolfi_index.has_package("gcc") {
            build_packages.push("gcc".to_string());
        }

        BuildTemplate {
            build_packages,
            build_commands: vec!["make".to_string()],
            cache_paths: vec![],
            artifacts: vec!["app".to_string()],
            common_ports: vec![],
            build_env: std::collections::HashMap::new(),
            runtime_copy: vec![],
            runtime_env: std::collections::HashMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }
}
