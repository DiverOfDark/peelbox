//! Bundler build system (Ruby)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct BundlerBuildSystem;

impl BuildSystem for BundlerBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Bundler
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "Gemfile".to_string(),
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, _manifest_content: Option<&str>) -> bool {
        manifest_name == "Gemfile"
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "ruby:3.2".to_string(),
            runtime_image: "ruby:3.2-slim".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec!["bundle install".to_string()],
            cache_paths: vec!["vendor/bundle/".to_string()],
            artifacts: vec![],
            common_ports: vec![3000],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["vendor".to_string(), ".bundle".to_string()]
    }
}
