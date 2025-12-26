//! Composer build system (PHP)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

fn parse_php_version(manifest_content: &str) -> Option<String> {
    let composer: serde_json::Value = serde_json::from_str(manifest_content).ok()?;
    let php_constraint = composer["require"]["php"].as_str()?;

    let version_str = php_constraint
        .trim()
        .trim_start_matches(">=")
        .trim_start_matches("^")
        .trim_start_matches("~")
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");

    if !version_str.is_empty() {
        Some(format!("php-{}", version_str))
    } else {
        None
    }
}

pub struct ComposerBuildSystem;

impl BuildSystem for ComposerBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Composer
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "composer.json".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "composer.lock".to_string(),
                priority: 12,
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
                Some("composer.lock") => true,
                Some("composer.json") => {
                    let abs_path = repo_root.join(rel_path);
                    let content = fs.read_to_string(&abs_path).ok();
                    if let Some(c) = content.as_deref() {
                        c.contains("\"name\"") && c.contains("\"require\"")
                    } else {
                        true
                    }
                }
                _ => false,
            };

            if is_match {
                detections.push(DetectionStack::new(
                    BuildSystemId::Composer,
                    LanguageId::PHP,
                    rel_path.clone(),
                ));
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
        let php_version = manifest_content
            .and_then(|c| parse_php_version(c))
            .or_else(|| wolfi_index.get_latest_version("php"))
            .expect("Failed to get php version from Wolfi index");

        BuildTemplate {
            build_packages: vec![php_version.clone(), "composer".to_string()],
            build_commands: vec!["composer install --no-dev --optimize-autoloader".to_string()],
            cache_paths: vec!["/root/.composer/cache/".to_string()],
            artifacts: vec!["vendor/".to_string(), "public/".to_string()],
            common_ports: vec![9000, 80],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".composer/cache".to_string(), "vendor".to_string()]
    }
}
