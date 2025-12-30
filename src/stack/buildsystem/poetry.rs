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

        // Derive version-specific pip package from Python version
        // python-3.14 -> py3.14-pip
        let pip_package = python_version
            .strip_prefix("python-")
            .map(|v| format!("py{}-pip", v))
            .unwrap_or_else(|| "py3-pip".to_string());

        let mut build_env = std::collections::HashMap::new();
        build_env.insert("POETRY_CACHE_DIR".to_string(), "/tmp/poetry-cache".to_string());
        build_env.insert("POETRY_VIRTUALENVS_IN_PROJECT".to_string(), "true".to_string());

        BuildTemplate {
            build_packages: vec![
                python_version.clone(),
                pip_package,
                "build-base".to_string(),
            ],
            build_commands: vec![
                "pip install --user poetry".to_string(),
                "/root/.local/bin/poetry install --compile --only main --no-root".to_string(),
            ],
            cache_paths: vec!["/tmp/poetry-cache/".to_string()],
            artifacts: vec![".".to_string(), ".venv/".to_string()],
            common_ports: vec![8000, 5000],
            build_env,
            runtime_copy: vec![
                (".".to_string(), "/app".to_string()),
                (".venv/".to_string(), "/app/.venv".to_string()),
            ],
            runtime_env: std::collections::HashMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".cache/pypoetry".to_string()]
    }
}
