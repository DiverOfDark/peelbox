//! Mix build system (Elixir)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct MixBuildSystem;

impl BuildSystem for MixBuildSystem {
    fn name(&self) -> &str {
        "mix"
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[ManifestPattern {
            filename: "mix.exs",
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, _manifest_content: Option<&str>) -> bool {
        manifest_name == "mix.exs"
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "elixir:1.15".to_string(),
            runtime_image: "elixir:1.15-slim".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec![
                "mix local.hex --force".to_string(),
                "mix local.rebar --force".to_string(),
                "mix deps.get".to_string(),
                "mix compile".to_string(),
            ],
            cache_paths: vec!["_build/".to_string(), "deps/".to_string()],
            artifacts: vec![],
            common_ports: vec![4000],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["_build".to_string(), "deps".to_string()]
    }
}
