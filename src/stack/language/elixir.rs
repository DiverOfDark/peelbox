//! Elixir language definition

use super::{Dependency, DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition};
use regex::Regex;

pub struct ElixirLanguage;

impl LanguageDefinition for ElixirLanguage {
    fn id(&self) -> crate::stack::LanguageId {
        crate::stack::LanguageId::Elixir
    }

    fn extensions(&self) -> Vec<String> {
        vec!["ex".to_string(), "exs".to_string()]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "mix.exs" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("defmodule") && content.contains("def project") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: crate::stack::BuildSystemId::Mix,
                    confidence,
                })
            }
            "mix.lock" => Some(DetectionResult {
                build_system: crate::stack::BuildSystemId::Mix,
                confidence: 1.0,
            }),
            _ => None,
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["mix".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec!["_build".to_string(), "deps".to_string(), "cover".to_string(), ".elixir_ls".to_string()]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // mix.exs: elixir: "~> 1.15"
        if let Some(caps) = Regex::new(r#"elixir:\s*"[^"]*(\d+\.\d+)"#)
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // .elixir-version file
        if !content.contains("defmodule") {
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

        if let Ok(re) = Regex::new(r#"\{:(\w+),\s*"([^"]+)"\}"#) {
            for cap in re.captures_iter(manifest_content) {
                if let (Some(name), Some(version)) = (cap.get(1), cap.get(2)) {
                    external_deps.push(Dependency {
                        name: name.as_str().to_string(),
                        version: Some(version.as_str().to_string()),
                        is_internal: false,
                    });
                }
            }
        }

        if let Ok(re) = Regex::new(r#"\{:(\w+),\s*path:\s*"([^"]+)"\}"#) {
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

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![(
            r#"System\.get_env\(["']([A-Z_][A-Z0-9_]*)["']"#.to_string(),
            "System.get_env".to_string(),
        )]
    }

    fn port_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r#"port:\s*(\d{4,5})"#.to_string(), "config".to_string()),
            (
                r#"Plug\.Cowboy\.http\([^,)]*,\s*port:\s*(\d{4,5})"#.to_string(),
                "Plug.Cowboy".to_string(),
            ),
        ]
    }

    fn health_check_patterns(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn default_health_endpoints(&self) -> Vec<(String, String)> {
        vec![]
    }

    fn default_env_vars(&self) -> Vec<String> {
        vec![]
    }

    fn is_main_file(&self, fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
        if let Some(path_str) = file_path.to_str() {
            if path_str.contains("/lib/") && path_str.ends_with("/application.ex") {
                return true;
            }
        }

        if let Ok(content) = fs.read_to_string(file_path) {
            if content.contains("def start(_type, _args)") || content.contains("use Application") {
                return true;
            }
        }

        false
    }

    fn runtime_name(&self) -> Option<String> {
        Some("elixir".to_string())
    }

    fn default_port(&self) -> Option<u16> {
        Some(4000)
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("mix phx.server".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions() {
        let lang = ElixirLanguage;
        assert!(lang.extensions().iter().any(|s| s == "ex"));
        assert!(lang.extensions().iter().any(|s| s == "exs"));
    }

    #[test]
    fn test_detect_mix_exs() {
        let lang = ElixirLanguage;
        let result = lang.detect("mix.exs", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::Mix);
    }

    #[test]
    fn test_detect_mix_exs_with_content() {
        let lang = ElixirLanguage;
        let content = r#"
defmodule MyApp.MixProject do
  use Mix.Project

  def project do
    [app: :my_app, version: "0.1.0"]
  end
end
"#;
        let result = lang.detect("mix.exs", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_mix_lock() {
        let lang = ElixirLanguage;
        let result = lang.detect("mix.lock", None);
        assert!(result.is_some());
    }

    #[test]
    fn test_compatible_build_systems() {
        let lang = ElixirLanguage;
        assert_eq!(lang.compatible_build_systems(), vec!["mix".to_string()]);
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = ElixirLanguage;
        assert!(lang.excluded_dirs().iter().any(|s| s == "_build"));
        assert!(lang.excluded_dirs().iter().any(|s| s == "deps"));
    }

    #[test]
    fn test_detect_version() {
        let lang = ElixirLanguage;
        let content = r#"
defmodule MyApp.MixProject do
  def project do
    [app: :my_app, elixir: "~> 1.15"]
  end
end
"#;
        assert_eq!(lang.detect_version(Some(content)), Some("1.15".to_string()));
    }

    #[test]
    fn test_parse_dependencies_version() {
        let lang = ElixirLanguage;
        let content = r#"
defp deps do
  [
    {:phoenix, "~> 1.7.0"},
    {:ecto, "~> 3.10"},
  ]
end
"#;
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 2);
        assert!(deps
            .external_deps
            .iter()
            .any(|d| d.name == "phoenix" && d.version == Some("~> 1.7.0".to_string())));
        assert!(deps.external_deps.iter().any(|d| d.name == "ecto"));
    }

    #[test]
    fn test_parse_dependencies_path() {
        let lang = ElixirLanguage;
        let content = r#"
defp deps do
  [
    {:my_lib, path: "../my_lib"},
    {:another_lib, path: "../another_lib"},
  ]
end
"#;
        let internal_paths = vec![std::path::PathBuf::from("../my_lib")];
        let deps = lang.parse_dependencies(content, &internal_paths);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.internal_deps.len(), 1);
        assert_eq!(deps.external_deps.len(), 1);
        assert!(deps
            .internal_deps
            .iter()
            .any(|d| d.name == "my_lib" && d.is_internal));
    }

    #[test]
    fn test_parse_dependencies_empty() {
        let lang = ElixirLanguage;
        let content = "defmodule MyApp.MixProject do\nend";
        let deps = lang.parse_dependencies(content, &[]);
        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert!(deps.external_deps.is_empty());
    }
}
