//! pip build system (Python)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct PipBuildSystem;

impl BuildSystem for PipBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Pip
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "requirements.txt".to_string(),
                priority: 8,
            },
            ManifestPattern {
                filename: "setup.py".to_string(),
                priority: 6,
            },
            ManifestPattern {
                filename: "setup.cfg".to_string(),
                priority: 5,
            },
        ]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for rel_path in file_tree {
            let filename = rel_path.file_name().and_then(|n| n.to_str());

            let is_match = match filename {
                Some("requirements.txt") => {
                    let abs_path = repo_root.join(rel_path);
                    let content = fs.read_to_string(&abs_path).ok();
                    if let Some(c) = content.as_deref() {
                        c.lines()
                            .any(|l| !l.trim().is_empty() && !l.starts_with('#'))
                    } else {
                        true
                    }
                }
                Some("setup.py") | Some("setup.cfg") => true,
                _ => false,
            };

            if is_match {
                detections.push(DetectionStack::new(
                    BuildSystemId::Pip,
                    LanguageId::Python,
                    repo_root.join(rel_path),
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
