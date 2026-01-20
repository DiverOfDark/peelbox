//! pip build system (Python)

use super::python_common::{parse_pyproject_toml_version, read_python_version_file};
use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::language::LanguageDefinition;
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
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
                        let has_deps = c
                            .lines()
                            .any(|l| !l.trim().is_empty() && !l.starts_with('#'));

                        if has_deps {
                            let lang = crate::language::PythonLanguage;
                            let project_dir = rel_path.parent().unwrap_or(Path::new(""));

                            if lang.is_runnable(
                                fs,
                                repo_root,
                                project_dir,
                                file_tree,
                                content.as_deref(),
                            ) {
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        let lang = crate::language::PythonLanguage;
                        let project_dir = rel_path.parent().unwrap_or(Path::new(""));
                        lang.is_runnable(fs, repo_root, project_dir, file_tree, None)
                    }
                }
                Some("setup.py") | Some("setup.cfg") => {
                    let lang = crate::language::PythonLanguage;
                    let project_dir = rel_path.parent().unwrap_or(Path::new(""));
                    let abs_path = repo_root.join(rel_path);
                    let content = fs.read_to_string(&abs_path).ok();

                    if lang.is_runnable(fs, repo_root, project_dir, file_tree, content.as_deref()) {
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };

            if is_match {
                detections.push(DetectionStack::new(
                    BuildSystemId::Pip,
                    LanguageId::Python,
                    rel_path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let python_version = read_python_version_file(service_path)
            .or_else(|| manifest_content.and_then(parse_pyproject_toml_version))
            .or_else(|| wolfi_index.get_latest_version("python"))
            .expect("Failed to get python version from Wolfi index");

        // Derive version-specific pip package from Python version
        // python-3.14 -> py3.14-pip
        let pip_package = python_version
            .strip_prefix("python-")
            .map(|v| format!("py{}-pip", v))
            .unwrap_or_else(|| "py3-pip".to_string());

        BuildTemplate {
            build_packages: vec![
                python_version.clone(),
                pip_package,
                "build-base".to_string(),
            ],
            build_commands: vec![
                "pip install --user --no-cache-dir -r requirements.txt".to_string()
            ],
            cache_paths: vec!["/root/.cache/pip/".to_string()],
            common_ports: vec![8000, 5000],
            build_env: std::collections::HashMap::new(),
            runtime_copy: vec![
                (".".to_string(), "/app".to_string()),
                ("/root/.local/".to_string(), "/root/.local".to_string()),
            ],
            runtime_env: {
                let mut env = std::collections::HashMap::new();
                env.insert(
                    "PYTHONPATH".to_string(),
                    format!(
                        "/root/.local/lib/{}/site-packages",
                        python_version
                            .strip_prefix("python-")
                            .map(|v| format!("python{}", v))
                            .unwrap_or_else(|| "python3.12".to_string())
                    ),
                );
                env.insert(
                    "PATH".to_string(),
                    "/root/.local/bin:/usr/local/bin:/usr/bin:/bin".to_string(),
                );
                env
            },
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".cache/pip".to_string()]
    }
}
