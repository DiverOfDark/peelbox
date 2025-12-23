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

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "gcc:latest".to_string(),
            runtime_image: "alpine:3.19".to_string(),
            build_packages: vec!["make".to_string(), "build-essential".to_string()],
            runtime_packages: vec![],
            build_commands: vec!["make".to_string()],
            cache_paths: vec![],
            artifacts: vec!["app".to_string()],
            common_ports: vec![],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }
}
