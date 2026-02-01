//! Ruby language definition

use super::{Dependency, DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition};
use regex::Regex;

pub struct RubyLanguage;

impl LanguageDefinition for RubyLanguage {
    fn id(&self) -> crate::LanguageId {
        crate::LanguageId::Ruby
    }

    fn extensions(&self) -> Vec<String> {
        vec!["rb".to_string(), "rake".to_string(), "gemspec".to_string()]
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
                    build_system: crate::BuildSystemId::Bundler,
                    confidence,
                })
            }
            "Gemfile.lock" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Bundler,
                confidence: 1.0,
            }),
            _ => None,
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["bundler".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec![
            "vendor".to_string(),
            "tmp".to_string(),
            "log".to_string(),
            "coverage".to_string(),
            ".bundle".to_string(),
        ]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
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

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        let mut external_deps = Vec::new();
        let mut internal_deps = Vec::new();

        if let Ok(re) = Regex::new(r#"gem\s+['"](\w+)['"],\s*path:\s*['"]([^'"]+)['"]"#) {
            for cap in re.captures_iter(manifest_content) {
                if let (Some(name), Some(path_match)) = (cap.get(1), cap.get(2)) {
                    let path_str = path_match.as_str();
                    let is_internal = all_internal_paths
                        .iter()
                        .any(|p| p.to_str().is_some_and(|s| s.contains(path_str)));

                    let dep = Dependency {
                        name: name.as_str().to_string(),
                        version: None,
                        is_internal,
                    };

                    if is_internal {
                        internal_deps.push(dep);
                    } else {
                        external_deps.push(dep);
                    }
                }
            }
        }

        if let Ok(re) = Regex::new(r#"gem\s+['"](\w+)['"](?:,\s*['"]([^'"]+)['"])?"#) {
            for cap in re.captures_iter(manifest_content) {
                if let Some(name) = cap.get(1) {
                    let name_str = name.as_str();
                    if internal_deps.iter().any(|d| d.name == name_str)
                        || external_deps.iter().any(|d| d.name == name_str)
                    {
                        continue;
                    }

                    let version = cap.get(2).map(|v| v.as_str().to_string());
                    external_deps.push(Dependency {
                        name: name_str.to_string(),
                        version,
                        is_internal: false,
                    });
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![(
            r#"ENV\[['"]([A-Z_][A-Z0-9_]*)['"]"#.to_string(),
            "ENV".to_string(),
        )]
    }

    fn port_patterns(&self) -> Vec<(String, String)> {
        vec![(r#"port:\s*(\d{4,5})"#.to_string(), "config".to_string())]
    }

    fn health_check_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r#"get\s+['"]([/\w\-]*health[/\w\-]*)['"]"#.to_string(),
                "Rails/Sinatra".to_string(),
            ),
            (
                r#"match\s+['"]([/\w\-]*health[/\w\-]*)['"]"#.to_string(),
                "Rails".to_string(),
            ),
        ]
    }

    fn default_health_endpoints(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn default_env_vars(&self) -> Vec<String> {
        vec![]
    }

    fn is_main_file(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        file_path: &std::path::Path,
    ) -> bool {
        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            if file_name == "config.ru" {
                return true;
            }
        }

        if let Some(path_str) = file_path.to_str() {
            if path_str.contains("/bin/") && path_str.ends_with(".rb") {
                return true;
            }
        }

        if let Ok(content) = fs.read_to_string(file_path) {
            if content.contains("Sinatra::Application.run!")
                || content.contains("Rails.application")
            {
                return true;
            }
        }

        false
    }

    fn runtime_name(&self) -> Option<String> {
        Some("ruby".to_string())
    }

    fn default_port(&self) -> Option<u16> {
        Some(3000)
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("ruby app.rb".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
        None
    }

    fn find_entrypoints(
        &self,
        _fs: &dyn peelbox_core::fs::FileSystem,
        _repo_root: &std::path::Path,
        _project_root: &std::path::Path,
        _file_tree: &[std::path::PathBuf],
    ) -> Vec<String> {
        vec![]
    }

    fn is_runnable(
        &self,
        _fs: &dyn peelbox_core::fs::FileSystem,
        _repo_root: &std::path::Path,
        _project_root: &std::path::Path,
        _file_tree: &[std::path::PathBuf],
        _manifest_content: Option<&str>,
    ) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions() {
        let lang = RubyLanguage;
        assert!(lang.extensions().iter().any(|s| s == "rb"));
    }

    #[test]
    fn test_detect_gemfile() {
        let lang = RubyLanguage;
        let result = lang.detect("Gemfile", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::BuildSystemId::Bundler);
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
    fn test_compatible_build_systems() {
        let lang = RubyLanguage;
        assert_eq!(lang.compatible_build_systems(), vec!["bundler".to_string()]);
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = RubyLanguage;
        assert!(lang.excluded_dirs().iter().any(|s| s == "vendor"));
        assert!(lang.excluded_dirs().iter().any(|s| s == ".bundle"));
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

    #[test]
    fn test_parse_dependencies_gems() {
        let lang = RubyLanguage;
        let content = r#"
source 'https://rubygems.org'

gem 'rails', '~> 7.0'
gem 'pg', '~> 1.5'
gem 'puma'
"#;
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 3);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "rails" && d.version == Some("~> 7.0".to_string())));
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "puma" && d.version.is_none()));
    }

    #[test]
    fn test_parse_dependencies_path() {
        let lang = RubyLanguage;
        let content = r#"
gem 'my_gem', path: '../my_gem'
gem 'another_gem', path: '../another_gem'
"#;
        let internal_paths = vec![std::path::PathBuf::from("../my_gem")];
        let deps = lang.parse_dependencies(content, &internal_paths);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.internal_deps.len(), 1);
        assert_eq!(deps.external_deps.len(), 1);
        assert!(deps
            .internal_deps
            .iter()
            .any(|d| d.name == "my_gem" && d.is_internal));
    }

    #[test]
    fn test_parse_dependencies_empty() {
        let lang = RubyLanguage;
        let content = "source 'https://rubygems.org'";
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert!(deps.external_deps.is_empty());
    }
}
