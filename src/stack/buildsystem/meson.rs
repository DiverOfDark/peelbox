//! Meson build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct MesonBuildSystem;

impl BuildSystem for MesonBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Meson
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "meson.build".to_string(),
            priority: 9,
        }]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "meson.build" {
            return false;
        }

        if let Some(content) = manifest_content {
            content.contains("project(")
        } else {
            true
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "gcc:latest".to_string(),
            runtime_image: "alpine:3.19".to_string(),
            build_packages: vec!["meson".to_string(), "ninja-build".to_string()],
            runtime_packages: vec![],
            build_commands: vec![
                "meson setup builddir".to_string(),
                "meson compile -C builddir".to_string(),
            ],
            cache_paths: vec![],
            artifacts: vec!["builddir/app".to_string()],
            common_ports: vec![],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["builddir".to_string()]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }
}
