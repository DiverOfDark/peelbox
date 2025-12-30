//! Go modules build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

fn parse_go_version(manifest_content: &str) -> Option<String> {
    for line in manifest_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("go ") {
            let version = trimmed.strip_prefix("go ")?.trim();
            let ver_num = version.split('.').take(2).collect::<Vec<_>>().join(".");
            if !ver_num.is_empty() {
                return Some(format!("go-{}", ver_num));
            }
        }
    }
    None
}

pub struct GoModBuildSystem;

impl BuildSystem for GoModBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::GoMod
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "go.mod".to_string(),
            priority: 10,
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
            if rel_path.file_name().and_then(|n| n.to_str()) == Some("go.mod") {
                let abs_path = repo_root.join(rel_path);
                let content = fs.read_to_string(&abs_path).ok();

                let is_valid = if let Some(c) = content.as_deref() {
                    c.contains("module ")
                } else {
                    true
                };

                if is_valid {
                    detections.push(DetectionStack::new(
                        BuildSystemId::GoMod,
                        LanguageId::Go,
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
        _service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let go_package = manifest_content
            .and_then(|c| parse_go_version(c))
            .or_else(|| wolfi_index.get_latest_version("go"))
            .or_else(|| {
                if wolfi_index.has_package("go") {
                    Some("go".to_string())
                } else {
                    None
                }
            })
            .expect("Failed to get go version from Wolfi index");

        let mut build_env = std::collections::HashMap::new();
        build_env.insert("GOCACHE".to_string(), ".cache/go-build".to_string());
        build_env.insert("GOMODCACHE".to_string(), ".cache/go-mod".to_string());
        build_env.insert("GOSUMDB".to_string(), "off".to_string());

        BuildTemplate {
            build_packages: vec![go_package],
            build_commands: vec![
                "go mod download".to_string(),
                "go build -o app .".to_string(),
            ],
            cache_paths: vec![
                ".cache/go-build".to_string(),
                ".cache/go-mod".to_string(),
            ],
            
            common_ports: vec![8080],
            build_env,
            runtime_copy: vec![("app".to_string(), "/usr/local/bin/app".to_string())],
            runtime_env: std::collections::HashMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![
            ".cache/go-build".to_string(),
            ".cache/go-mod".to_string(),
        ]
    }
    fn workspace_configs(&self) -> Vec<String> {
        vec!["go.work".to_string()]
    }
}
