//! Composer build system (PHP)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct ComposerBuildSystem;

impl BuildSystem for ComposerBuildSystem {
    fn name(&self) -> &str {
        "composer"
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "composer.json",
                priority: 10,
            },
            ManifestPattern {
                filename: "composer.lock",
                priority: 12,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        match manifest_name {
            "composer.lock" => true,
            "composer.json" => {
                if let Some(content) = manifest_content {
                    content.contains("\"name\"") && content.contains("\"require\"")
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "composer:2".to_string(),
            runtime_image: "php:8.2-fpm".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
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
