//! Go language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};
use regex::Regex;

pub struct GoLanguage;

impl LanguageDefinition for GoLanguage {
    fn name(&self) -> &str {
        "Go"
    }

    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[ManifestPattern {
            filename: "go.mod",
            build_system: "go",
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
        if manifest_name != "go.mod" {
            return None;
        }

        let mut confidence = 0.9;
        if let Some(content) = manifest_content {
            if content.contains("module ") {
                confidence = 1.0;
            }
        }

        Some(DetectionResult {
            build_system: "go".to_string(),
            confidence,
        })
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        if build_system != "go" {
            return None;
        }

        Some(BuildTemplate {
            build_image: "golang:1.21".to_string(),
            runtime_image: "alpine:3.19".to_string(),
            build_packages: vec![],
            runtime_packages: vec!["ca-certificates".to_string()],
            build_commands: vec!["go build -o app .".to_string()],
            cache_paths: vec![
                "/go/pkg/mod/".to_string(),
                "/root/.cache/go-build/".to_string(),
            ],
            artifacts: vec!["app".to_string()],
            common_ports: vec![8080],
        })
    }

    fn build_systems(&self) -> &[&str] {
        &["go"]
    }

    fn excluded_dirs(&self) -> &[&str] {
        &["vendor"]
    }

    fn workspace_configs(&self) -> &[&str] {
        &["go.work"]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // go.mod: go 1.21
        if let Some(caps) = Regex::new(r"(?m)^go\s+(\d+\.\d+)")
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
        let lang = GoLanguage;
        assert_eq!(lang.name(), "Go");
    }

    #[test]
    fn test_extensions() {
        let lang = GoLanguage;
        assert_eq!(lang.extensions(), &["go"]);
    }

    #[test]
    fn test_detect_go_mod() {
        let lang = GoLanguage;
        let result = lang.detect("go.mod", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "go");
    }

    #[test]
    fn test_detect_go_mod_with_content() {
        let lang = GoLanguage;
        let content = "module github.com/user/project\n\ngo 1.21";
        let result = lang.detect("go.mod", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_build_template() {
        let lang = GoLanguage;
        let template = lang.build_template("go");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("golang"));
        assert_eq!(t.runtime_image, "alpine:3.19");
    }

    #[test]
    fn test_workspace_configs() {
        let lang = GoLanguage;
        assert!(lang.workspace_configs().contains(&"go.work"));
    }

    #[test]
    fn test_detect_version() {
        let lang = GoLanguage;
        let content = "module github.com/user/project\n\ngo 1.21";
        assert_eq!(lang.detect_version(Some(content)), Some("1.21".to_string()));
    }
}
