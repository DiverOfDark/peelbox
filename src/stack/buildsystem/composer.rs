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

        let required_extensions = vec!["ctype", "phar", "openssl", "mbstring", "xml", "dom"];
        let extension_packages: Vec<String> = required_extensions
            .iter()
            .map(|ext| format!("{}-{}", php_version, ext))
            .collect();

        let mut build_packages = vec![php_version.clone(), "composer".to_string()];
        build_packages.extend(extension_packages);

        BuildTemplate {
            build_packages,
            build_commands: vec![
                "composer config allow-plugins.symfony/runtime true".to_string(),
                "composer install --no-dev --optimize-autoloader --ignore-platform-reqs".to_string(),
            ],
            cache_paths: vec!["/root/.composer/cache/".to_string()],
            artifacts: vec!["vendor/".to_string(), "bin/".to_string(), "public/".to_string(), "src/".to_string(), "config/".to_string()],
            common_ports: vec![9000, 80],
            build_env: std::collections::HashMap::new(),
            runtime_copy: vec![
                ("vendor/".to_string(), "/app/vendor".to_string()),
                ("bin/".to_string(), "/app/bin".to_string()),
                ("public/".to_string(), "/app/public".to_string()),
                ("src/".to_string(), "/app/src".to_string()),
                ("config/".to_string(), "/app/config".to_string()),
            ],
            runtime_env: std::collections::HashMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".composer/cache".to_string(), "vendor".to_string()]
    }
}
