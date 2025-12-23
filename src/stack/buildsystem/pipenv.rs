//! Pipenv build system (Python)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct PipenvBuildSystem;

impl BuildSystem for PipenvBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Pipenv
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "Pipfile".to_string(),
            priority: 10,
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
            if path.file_name().and_then(|n| n.to_str()) == Some("Pipfile") {
                detections.push(DetectionStack::new(
                    BuildSystemId::Pipenv,
                    LanguageId::Python,
                    path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "python:3.11".to_string(),
            runtime_image: "python:3.11-slim".to_string(),
            build_packages: vec!["build-essential".to_string()],
            runtime_packages: vec![],
            build_commands: vec![
                "pip install pipenv".to_string(),
                "pipenv install --deploy".to_string(),
            ],
            cache_paths: vec![
                "/root/.cache/pip/".to_string(),
                "/root/.cache/pipenv/".to_string(),
            ],
            artifacts: vec!["Pipfile".to_string()],
            common_ports: vec![8000, 5000],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".cache/pip".to_string(), ".cache/pipenv".to_string()]
    }
}
