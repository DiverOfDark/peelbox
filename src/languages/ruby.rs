//! Ruby language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};

pub struct RubyLanguage;

impl LanguageDefinition for RubyLanguage {
    fn name(&self) -> &str {
        "Ruby"
    }

    fn extensions(&self) -> &[&str] {
        &["rb", "rake", "gemspec"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "Gemfile",
                build_system: "bundler",
                priority: 10,
            },
            ManifestPattern {
                filename: "Gemfile.lock",
                build_system: "bundler",
                priority: 12,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
        match manifest_name {
            "Gemfile" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("source") && content.contains("gem ") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: "bundler".to_string(),
                    confidence,
                })
            }
            "Gemfile.lock" => Some(DetectionResult {
                build_system: "bundler".to_string(),
                confidence: 1.0,
            }),
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        if build_system != "bundler" {
            return None;
        }

        Some(BuildTemplate {
            build_image: "ruby:3.2".to_string(),
            runtime_image: "ruby:3.2-slim".to_string(),
            build_packages: vec!["build-essential".to_string()],
            runtime_packages: vec![],
            build_commands: vec![
                "bundle config set --local deployment 'true'".to_string(),
                "bundle install".to_string(),
            ],
            cache_paths: vec!["vendor/bundle/".to_string()],
            artifacts: vec!["vendor/bundle/".to_string(), "app/".to_string()],
            common_ports: vec![3000],
        })
    }

    fn build_systems(&self) -> &[&str] {
        &["bundler"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = RubyLanguage;
        assert_eq!(lang.name(), "Ruby");
    }

    #[test]
    fn test_extensions() {
        let lang = RubyLanguage;
        assert!(lang.extensions().contains(&"rb"));
    }

    #[test]
    fn test_detect_gemfile() {
        let lang = RubyLanguage;
        let result = lang.detect("Gemfile", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "bundler");
    }

    #[test]
    fn test_detect_gemfile_with_content() {
        let lang = RubyLanguage;
        let content = r#"
source 'https://rubygems.org'

gem 'rails', '~> 7.0'
"#;
        let result = lang.detect("Gemfile", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_gemfile_lock() {
        let lang = RubyLanguage;
        let result = lang.detect("Gemfile.lock", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_build_template() {
        let lang = RubyLanguage;
        let template = lang.build_template("bundler");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("ruby"));
        assert!(t.build_commands.iter().any(|c| c.contains("bundle")));
    }
}
