//! PHP language definition

use super::{
    Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition,
};
use regex::Regex;

pub struct PhpLanguage;

impl LanguageDefinition for PhpLanguage {
    fn name(&self) -> &str {
        "PHP"
    }

    fn extensions(&self) -> &[&str] {
        &["php", "phtml"]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
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

    fn compatible_build_systems(&self) -> &[&str] {
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

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        let mut external_deps = Vec::new();
        let mut internal_deps = Vec::new();

        let parsed: Result<serde_json::Value, _> = serde_json::from_str(manifest_content);
        if let Ok(json) = parsed {
            for section in ["require", "require-dev"] {
                if let Some(deps) = json.get(section).and_then(|v| v.as_object()) {
                    for (name, version) in deps {
                        if name == "php" || name.starts_with("ext-") {
                            continue;
                        }

                        external_deps.push(Dependency {
                            name: name.clone(),
                            version: version.as_str().map(|s| s.to_string()),
                            is_internal: false,
                        });
                    }
                }
            }

            if let Some(repos) = json.get("repositories").and_then(|v| v.as_array()) {
                for repo in repos {
                    if let Some(repo_type) = repo.get("type").and_then(|v| v.as_str()) {
                        if repo_type == "path" {
                            if let Some(url) = repo.get("url").and_then(|v| v.as_str()) {
                                let is_internal = all_internal_paths
                                    .iter()
                                    .any(|p| p.to_str().is_some_and(|s| s.contains(url)));

                                if is_internal {
                                    let name = std::path::Path::new(url)
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or(url)
                                        .to_string();

                                    internal_deps.push(Dependency {
                                        name,
                                        version: None,
                                        is_internal: true,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r#"getenv\(['"]([A-Z_][A-Z0-9_]*)['"]"#, "getenv")]
    }

    fn port_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r#"['"]SERVER_PORT['"].*?(\d{4,5})"#, "server port")]
    }

    fn health_check_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r#"\$app->get\(['"]([/\w\-]*health[/\w\-]*)['"]"#, "Slim"),
            (r#"Route::get\(['"]([/\w\-]*health[/\w\-]*)['"]"#, "Laravel"),
        ]
    }

    fn is_main_file(&self, _fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            if file_name == "index.php" {
                return true;
            }
        }

        if let Some(path_str) = file_path.to_str() {
            if path_str.contains("/public/index.php") || path_str.contains("/bin/") {
                return true;
            }
        }

        false
    }

    fn runtime_name(&self) -> Option<&'static str> {
        Some("php")
    }

    fn default_port(&self) -> Option<u16> {
        Some(8000)
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("php -S 0.0.0.0:8000 public/index.php".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
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
    fn test_compatible_build_systems() {
        let lang = PhpLanguage;
        assert_eq!(lang.compatible_build_systems(), &["composer"]);
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

    #[test]
    fn test_parse_dependencies_require() {
        let lang = PhpLanguage;
        let content = r#"{
  "require": {
    "php": ">=8.0",
    "laravel/framework": "^10.0",
    "guzzlehttp/guzzle": "^7.5"
  }
}"#;
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "laravel/framework"));
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "guzzlehttp/guzzle"));
    }

    #[test]
    fn test_parse_dependencies_path_repositories() {
        let lang = PhpLanguage;
        let content = r#"{
  "require": {"vendor/package": "*"},
  "repositories": [
    {"type": "path", "url": "../my-package"},
    {"type": "vcs", "url": "https://github.com/vendor/package"}
  ]
}"#;
        let internal_paths = vec![std::path::PathBuf::from("../my-package")];
        let deps = lang.parse_dependencies(content, &internal_paths);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.internal_deps.len(), 1);
        assert!(deps
            .internal_deps
            .iter()
            .any(|d| d.name == "my-package" && d.is_internal));
    }

    #[test]
    fn test_parse_dependencies_invalid_json() {
        let lang = PhpLanguage;
        let content = "not json";
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert!(deps.external_deps.is_empty());
    }
}
