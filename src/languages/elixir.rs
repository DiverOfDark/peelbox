//! Elixir language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};

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

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
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
}
