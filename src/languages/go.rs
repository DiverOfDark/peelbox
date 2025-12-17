//! Go language definition

use super::{
    Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition,
};
use regex::Regex;
use std::collections::HashSet;

pub struct GoLanguage;

impl LanguageDefinition for GoLanguage {
    fn id(&self) -> crate::stack::LanguageId {
        crate::stack::LanguageId::Go
    }

    fn extensions(&self) -> &[&str] {
        &["go"]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
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

    fn compatible_build_systems(&self) -> &[&str] {
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

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        _all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        let mut internal_deps = Vec::new();
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();
        let mut replace_map = std::collections::HashMap::new();

        let require_re =
            Regex::new(r"(?m)^\s*([^\s]+)\s+v?([^\s]+)").expect("require regex is valid");
        let replace_re =
            Regex::new(r"(?m)^\s*([^\s]+)\s+=>\s+([^\s]+)").expect("replace regex is valid");

        let mut in_require = false;
        let mut in_replace = false;

        for line in manifest_content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("require (") {
                in_require = true;
                continue;
            } else if trimmed.starts_with("replace (") {
                in_replace = true;
                continue;
            } else if trimmed == ")" {
                in_require = false;
                in_replace = false;
                continue;
            }

            let (directive_type, content_slice) = if in_require {
                ("require", trimmed)
            } else if in_replace {
                ("replace", trimmed)
            } else if let Some(content) = trimmed.strip_prefix("require ") {
                ("require", content)
            } else if let Some(content) = trimmed.strip_prefix("replace ") {
                ("replace", content)
            } else {
                continue;
            };

            match directive_type {
                "require" => {
                    self.parse_require_directive(
                        &require_re,
                        content_slice,
                        &mut seen,
                        &mut external_deps,
                    );
                }
                "replace" => {
                    self.parse_replace_directive(&replace_re, content_slice, &mut replace_map);
                }
                _ => {}
            }
        }

        for (original, replacement) in replace_map {
            if replacement.starts_with("./") || replacement.starts_with("../") {
                if let Some(idx) = external_deps.iter().position(|d| d.name == original) {
                    let dep = external_deps.remove(idx);
                    internal_deps.push(Dependency {
                        name: dep.name,
                        version: Some(replacement),
                        is_internal: true,
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

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r#"os\.Getenv\(["']([A-Z_][A-Z0-9_]*)["']"#, "os.Getenv"),
            (r#"viper\.GetString\(["']([A-Z_][A-Z0-9_]*)["']"#, "viper"),
        ]
    }

    fn port_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"\.Run\([^:)]*:(\d{4,5})", "gin.Run()"),
            (
                r#"http\.ListenAndServe\([^:)]*:(\d{4,5})"#,
                "http.ListenAndServe",
            ),
        ]
    }

    fn health_check_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r#"router\.GET\(['"]([/\w\-]*health[/\w\-]*)['"]"#, "Gin")]
    }

    fn default_health_endpoints(&self) -> Vec<(&'static str, &'static str)> {
        vec![("/health", "Gin"), ("/healthz", "Kubernetes")]
    }

    fn default_env_vars(&self) -> Vec<&'static str> {
        vec![]
    }

    fn is_main_file(&self, fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
        if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
            if file_name == "main.go" {
                return true;
            }
        }

        if let Some(path_str) = file_path.to_str() {
            if path_str.contains("/cmd/") && path_str.ends_with("/main.go") {
                return true;
            }
        }

        if let Ok(content) = fs.read_to_string(file_path) {
            if content.contains("func main()") {
                return true;
            }
        }

        false
    }

    fn runtime_name(&self) -> Option<&'static str> {
        Some("go")
    }

    fn default_port(&self) -> Option<u16> {
        Some(8080)
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("./app".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
        None
    }
}

impl GoLanguage {
    fn parse_require_directive(
        &self,
        require_re: &Regex,
        content: &str,
        seen: &mut HashSet<String>,
        external_deps: &mut Vec<Dependency>,
    ) {
        if let Some(caps) = require_re.captures(content) {
            if let (Some(name), Some(version)) = (caps.get(1), caps.get(2)) {
                let name_str = name.as_str().to_string();
                if !seen.contains(&name_str) {
                    seen.insert(name_str.clone());
                    external_deps.push(Dependency {
                        name: name_str,
                        version: Some(version.as_str().to_string()),
                        is_internal: false,
                    });
                }
            }
        }
    }

    fn parse_replace_directive(
        &self,
        replace_re: &Regex,
        content: &str,
        replace_map: &mut std::collections::HashMap<String, String>,
    ) {
        if let Some(caps) = replace_re.captures(content) {
            if let (Some(original), Some(replacement)) = (caps.get(1), caps.get(2)) {
                replace_map.insert(
                    original.as_str().to_string(),
                    replacement.as_str().to_string(),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_compatible_build_systems() {
        let lang = GoLanguage;
        assert_eq!(lang.compatible_build_systems(), &["go"]);
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

    #[test]
    fn test_parse_dependencies_simple() {
        let lang = GoLanguage;
        let content = r#"
module github.com/user/project

go 1.21

require (
    github.com/gin-gonic/gin v1.9.0
    github.com/lib/pq v1.10.7
)
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "github.com/gin-gonic/gin"));
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "github.com/lib/pq"));
    }

    #[test]
    fn test_parse_dependencies_replace() {
        let lang = GoLanguage;
        let content = r#"
module github.com/user/project

require (
    github.com/user/shared v1.0.0
    github.com/gin-gonic/gin v1.9.0
)

replace github.com/user/shared => ../shared
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.internal_deps.len(), 1);
        assert_eq!(deps.internal_deps[0].name, "github.com/user/shared");
        assert!(deps.internal_deps[0].is_internal);
    }

    #[test]
    fn test_parse_dependencies_replace_block() {
        let lang = GoLanguage;
        let content = r#"
module github.com/user/project

require (
    github.com/user/lib1 v1.0.0
    github.com/user/lib2 v1.0.0
)

replace (
    github.com/user/lib1 => ./lib1
    github.com/user/lib2 => ./lib2
)
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.internal_deps.len(), 2);
        assert!(deps
            .internal_deps
            .iter()
            .any(|d| d.name == "github.com/user/lib1"));
        assert!(deps
            .internal_deps
            .iter()
            .any(|d| d.name == "github.com/user/lib2"));
    }
}
