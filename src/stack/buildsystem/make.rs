//! Make build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct MakeBuildSystem;

impl BuildSystem for MakeBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Make
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "Makefile".to_string(),
            priority: 8,
        }]
    }

    fn detect(&self, manifest_name: &str, _manifest_content: Option<&str>) -> bool {
        manifest_name == "Makefile" || manifest_name == "makefile"
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "gcc:latest".to_string(),
            runtime_image: "alpine:3.19".to_string(),
            build_packages: vec!["make".to_string(), "build-essential".to_string()],
            runtime_packages: vec![],
            build_commands: vec!["make".to_string()],
            cache_paths: vec![],
            artifacts: vec!["app".to_string()],
            common_ports: vec![],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }
}
