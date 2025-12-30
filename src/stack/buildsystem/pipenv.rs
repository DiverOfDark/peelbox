//! Pipenv build system (Python)

use super::python_common::{parse_pyproject_toml_version, read_python_version_file};
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

    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let python_version = read_python_version_file(service_path)
            .or_else(|| manifest_content.and_then(|c| parse_pyproject_toml_version(c)))
            .or_else(|| wolfi_index.get_latest_version("python"))
            .expect("Failed to get python version from Wolfi index");

        let mut build_env = std::collections::HashMap::new();
        build_env.insert("PIPENV_CACHE_DIR".to_string(), "/root/.cache/pipenv".to_string());

        BuildTemplate {
            build_packages: vec![
                python_version.clone(),
                "py3-pip".to_string(),
                "build-base".to_string(),
            ],
            build_commands: vec![
                "pip install --break-system-packages pipenv".to_string(),
                "pipenv install --deploy".to_string(),
            ],
            cache_paths: vec![
                "/root/.cache/pip/".to_string(),
                "/root/.cache/pipenv/".to_string(),
            ],
            
            common_ports: vec![8000, 5000],
            build_env,
            runtime_copy: vec![],
            runtime_env: std::collections::HashMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".cache/pip".to_string(), ".cache/pipenv".to_string()]
    }
}
