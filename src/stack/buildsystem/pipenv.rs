//! Pipenv build system (Python)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct PipenvBuildSystem;

impl BuildSystem for PipenvBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Pipenv
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "Pipfile".to_string(),
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, _manifest_content: Option<&str>) -> bool {
        manifest_name == "Pipfile"
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
