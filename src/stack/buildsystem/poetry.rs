//! Poetry build system (Python)

use super::python_common::{parse_pyproject_toml_version, read_python_version_file};
use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct PoetryBuildSystem;

impl BuildSystem for PoetryBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Poetry
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "pyproject.toml".to_string(),
            priority: 12,
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
            if rel_path.file_name().and_then(|n| n.to_str()) == Some("pyproject.toml") {
                let abs_path = repo_root.join(rel_path);
                let content = fs.read_to_string(&abs_path).ok();

                let is_valid = if let Some(c) = content.as_deref() {
                    c.contains("[tool.poetry]")
                } else {
                    true
                };

                if is_valid {
                    detections.push(DetectionStack::new(
                        BuildSystemId::Poetry,
                        LanguageId::Python,
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
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let python_version = read_python_version_file(service_path)
            .or_else(|| manifest_content.and_then(|c| parse_pyproject_toml_version(c)))
            .or_else(|| wolfi_index.get_latest_version("python"))
            .expect("Failed to get python version from Wolfi index");

        BuildTemplate {
            build_packages: vec![python_version.clone(), "build-base".to_string()],
            runtime_packages: vec![python_version],
            build_commands: vec![
                "pip install poetry".to_string(),
                "poetry install --no-dev".to_string(),
            ],
            cache_paths: vec![".venv/".to_string(), "/root/.cache/pypoetry/".to_string()],
            artifacts: vec!["dist/".to_string(), ".venv/".to_string()],
            common_ports: vec![8000, 5000],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".venv".to_string(), ".cache/pypoetry".to_string()]
    }
}
