//! Ruby language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};
use regex::Regex;

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

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
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

    fn excluded_dirs(&self) -> &[&str] {
        &["vendor", "tmp", "log", "coverage", ".bundle"]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // Gemfile: ruby "3.2.0"
        if let Some(caps) = Regex::new(r#"ruby\s+["'](\d+\.\d+)"#)
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // .ruby-version file (just contains version)
        if !content.contains("source") && !content.contains("gem ") {
            let trimmed = content.trim();
            if let Some(caps) = Regex::new(r"^(\d+\.\d+)").ok()?.captures(trimmed) {
                return Some(caps.get(1)?.as_str().to_string());
            }
        }

        None
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

    #[test]
    fn test_excluded_dirs() {
        let lang = RubyLanguage;
        assert!(lang.excluded_dirs().contains(&"vendor"));
        assert!(lang.excluded_dirs().contains(&".bundle"));
    }

    #[test]
    fn test_detect_version_gemfile() {
        let lang = RubyLanguage;
        let content = r#"source 'https://rubygems.org'
ruby "3.2.0"
"#;
        assert_eq!(lang.detect_version(Some(content)), Some("3.2".to_string()));
    }

    #[test]
    fn test_detect_version_ruby_version_file() {
        let lang = RubyLanguage;
        assert_eq!(lang.detect_version(Some("3.1.4")), Some("3.1".to_string()));
    }
}
