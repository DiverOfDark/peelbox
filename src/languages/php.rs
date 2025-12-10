//! PHP language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};
use regex::Regex;

pub struct PhpLanguage;

impl LanguageDefinition for PhpLanguage {
    fn name(&self) -> &str {
        "PHP"
    }

    fn extensions(&self) -> &[&str] {
        &["php", "phtml"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "composer.json",
                build_system: "composer",
                priority: 10,
            },
            ManifestPattern {
                filename: "composer.lock",
                build_system: "composer",
                priority: 12,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
        match manifest_name {
            "composer.json" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("\"name\"") && content.contains("\"require\"") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: "composer".to_string(),
                    confidence,
                })
            }
            "composer.lock" => Some(DetectionResult {
                build_system: "composer".to_string(),
                confidence: 1.0,
            }),
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        if build_system != "composer" {
            return None;
        }

        Some(BuildTemplate {
            build_image: "composer:2".to_string(),
            runtime_image: "php:8.2-fpm".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec![
                "composer install --no-dev --optimize-autoloader".to_string(),
            ],
            cache_paths: vec!["/root/.composer/cache/".to_string()],
            artifacts: vec!["vendor/".to_string(), "public/".to_string()],
            common_ports: vec![9000, 80],
        })
    }

    fn build_systems(&self) -> &[&str] {
        &["composer"]
    }

    fn excluded_dirs(&self) -> &[&str] {
        &["vendor", "storage", "bootstrap/cache"]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // composer.json: "php": ">=8.2"
        if let Some(caps) = Regex::new(r#""php"\s*:\s*"[^"]*(\d+\.\d+)"#)
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = PhpLanguage;
        assert_eq!(lang.name(), "PHP");
    }

    #[test]
    fn test_extensions() {
        let lang = PhpLanguage;
        assert!(lang.extensions().contains(&"php"));
    }

    #[test]
    fn test_detect_composer_json() {
        let lang = PhpLanguage;
        let result = lang.detect("composer.json", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "composer");
    }

    #[test]
    fn test_detect_composer_json_with_content() {
        let lang = PhpLanguage;
        let content = r#"{"name": "vendor/project", "require": {"php": ">=8.0"}}"#;
        let result = lang.detect("composer.json", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_composer_lock() {
        let lang = PhpLanguage;
        let result = lang.detect("composer.lock", None);
        assert!(result.is_some());
    }

    #[test]
    fn test_build_template() {
        let lang = PhpLanguage;
        let template = lang.build_template("composer");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("composer"));
        assert!(t.runtime_image.contains("php"));
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = PhpLanguage;
        assert!(lang.excluded_dirs().contains(&"vendor"));
    }

    #[test]
    fn test_detect_version() {
        let lang = PhpLanguage;
        let content = r#"{"require": {"php": ">=8.2"}}"#;
        assert_eq!(lang.detect_version(Some(content)), Some("8.2".to_string()));
    }
}
