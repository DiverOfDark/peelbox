//! Poetry build system (Python)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct PoetryBuildSystem;

impl BuildSystem for PoetryBuildSystem {
    fn name(&self) -> &str {
        "poetry"
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[ManifestPattern {
            filename: "pyproject.toml",
            priority: 12,
        }]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "pyproject.toml" {
            return false;
        }

        if let Some(content) = manifest_content {
            content.contains("[tool.poetry]")
        } else {
            false
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "python:3.11".to_string(),
            runtime_image: "python:3.11-slim".to_string(),
            build_packages: vec!["build-essential".to_string()],
            runtime_packages: vec![],
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
