//! Elixir language definition

use super::{
    BuildTemplate, Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition, ManifestPattern,
};
use regex::Regex;

pub struct ElixirLanguage;

impl LanguageDefinition for ElixirLanguage {
    fn name(&self) -> &str {
        "Elixir"
    }

    fn extensions(&self) -> &[&str] {
        &["ex", "exs"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "mix.exs",
                build_system: "mix",
                priority: 10,
            },
            ManifestPattern {
                filename: "mix.lock",
                build_system: "mix",
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
            "mix.exs" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content.contains("defmodule") && content.contains("def project") {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: "mix".to_string(),
                    confidence,
                })
            }
            "mix.lock" => Some(DetectionResult {
                build_system: "mix".to_string(),
                confidence: 1.0,
            }),
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        if build_system != "mix" {
            return None;
        }

        Some(BuildTemplate {
            build_image: "elixir:1.15".to_string(),
            runtime_image: "elixir:1.15-slim".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec![
                "mix local.hex --force".to_string(),
                "mix local.rebar --force".to_string(),
                "mix deps.get --only prod".to_string(),
                "MIX_ENV=prod mix compile".to_string(),
                "MIX_ENV=prod mix release".to_string(),
            ],
            cache_paths: vec!["deps/".to_string(), "_build/".to_string()],
            artifacts: vec!["_build/prod/rel/".to_string()],
            common_ports: vec![4000],
        })
    }

    fn build_systems(&self) -> &[&str] {
        &["mix"]
    }

    fn excluded_dirs(&self) -> &[&str] {
        &["_build", "deps", "cover", ".elixir_ls"]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = ElixirLanguage;
        assert_eq!(lang.name(), "Elixir");
    }

    #[test]
    fn test_extensions() {
        let lang = ElixirLanguage;
        assert!(lang.extensions().contains(&"ex"));
        assert!(lang.extensions().contains(&"exs"));
    }

    #[test]
    fn test_detect_mix_exs() {
        let lang = ElixirLanguage;
        let result = lang.detect("mix.exs", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "mix");
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
    fn test_build_template() {
        let lang = ElixirLanguage;
        let template = lang.build_template("mix");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("elixir"));
        assert!(t.build_commands.iter().any(|c| c.contains("mix")));
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = ElixirLanguage;
        assert!(lang.excluded_dirs().contains(&"_build"));
        assert!(lang.excluded_dirs().contains(&"deps"));
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
