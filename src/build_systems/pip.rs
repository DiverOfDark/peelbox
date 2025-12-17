//! pip build system (Python)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct PipBuildSystem;

impl BuildSystem for PipBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Pip
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "requirements.txt",
                priority: 8,
            },
            ManifestPattern {
                filename: "setup.py",
                priority: 6,
            },
            ManifestPattern {
                filename: "setup.cfg",
                priority: 5,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        match manifest_name {
            "requirements.txt" => {
                if let Some(content) = manifest_content {
                    content
                        .lines()
                        .any(|l| !l.trim().is_empty() && !l.starts_with('#'))
                } else {
                    true
                }
            }
            "setup.py" | "setup.cfg" => true,
            _ => false,
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "python:3.11".to_string(),
            runtime_image: "python:3.11-slim".to_string(),
            build_packages: vec!["build-essential".to_string()],
            runtime_packages: vec![],
            build_commands: vec!["pip install --no-cache-dir -r requirements.txt".to_string()],
            cache_paths: vec!["/root/.cache/pip/".to_string()],
            artifacts: vec![
                "/usr/local/lib/python3.11/site-packages".to_string(),
                "app/".to_string(),
            ],
            common_ports: vec![8000, 5000],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".cache/pip".to_string()]
    }
}
