//! Bazel build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

pub struct BazelBuildSystem;

impl BuildSystem for BazelBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Bazel
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "WORKSPACE".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "BUILD".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "BUILD.bazel".to_string(),
                priority: 10,
            },
        ]
    }

    fn detect_all(
        &self,
        _repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for path in file_tree {
            let name = path.file_name().and_then(|n| n.to_str());
            if name == Some("BUILD") || name == Some("BUILD.bazel") {
                let mut lang_id = LanguageId::Custom("Bazel".to_string());
                if let Some(parent) = path.parent() {
                    for p in file_tree {
                        if p.starts_with(parent) {
                            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                                if ext == "cc" || ext == "cpp" {
                                    lang_id = LanguageId::Cpp;
                                    break;
                                } else if ext == "java" {
                                    lang_id = LanguageId::Java;
                                    break;
                                }
                            }
                        }
                    }
                }

                detections.push(DetectionStack::new(
                    BuildSystemId::Bazel,
                    lang_id,
                    path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _relative_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let mut build_packages = vec!["build-base".to_string()];

        if wolfi_index.has_package("bazel-7") {
            build_packages.push("bazel-7".to_string());
        } else if wolfi_index.has_package("bazel") {
            build_packages.push("bazel".to_string());
        } else {
            build_packages.push("bazel-7".to_string());
        }

        if wolfi_index.has_package("openjdk-21") {
            build_packages.push("openjdk-21".to_string());
        }

        let mut build_env = std::collections::BTreeMap::new();
        build_env.insert(
            "JAVA_HOME".to_string(),
            "/usr/lib/jvm/java-21-openjdk".to_string(),
        );

        BuildTemplate {
            build_packages,
            build_commands: vec!["bazel build //...".to_string()],
            cache_paths: vec!["~/.cache/bazel".to_string()],
            common_ports: vec![8080],
            build_env,
            runtime_copy: vec![(
                "bazel-bin/{project_name}".to_string(),
                "/usr/local/bin/{project_name}".to_string(),
            )],
            runtime_env: std::collections::BTreeMap::new(),
            runtime_workdir: Some("/app".to_string()),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![]
    }

    fn parse_package_metadata(
        &self,
        manifest_content: &str,
    ) -> Result<(String, bool), anyhow::Error> {
        let re = regex::Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap();
        if let Some(cap) = re.captures(manifest_content) {
            if let Some(name) = cap.get(1) {
                return Ok((name.as_str().to_string(), true));
            }
        }
        Ok(("app".to_string(), true))
    }
}
