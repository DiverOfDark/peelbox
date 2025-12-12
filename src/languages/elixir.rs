//! Elixir language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};
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
}
