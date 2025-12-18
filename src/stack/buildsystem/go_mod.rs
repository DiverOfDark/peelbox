//! Go modules build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct GoModBuildSystem;

impl BuildSystem for GoModBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::GoMod
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[ManifestPattern {
            filename: "go.mod",
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "go.mod" {
            return false;
        }

        if let Some(content) = manifest_content {
            content.contains("module ")
        } else {
            true
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "golang:1.21".to_string(),
            runtime_image: "alpine:3.19".to_string(),
            build_packages: vec![],
            runtime_packages: vec!["ca-certificates".to_string()],
            build_commands: vec!["go build -o app .".to_string()],
            cache_paths: vec![
                "/go/pkg/mod/".to_string(),
                "/root/.cache/go-build/".to_string(),
            ],
            artifacts: vec!["app".to_string()],
            common_ports: vec![8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".cache/go-build".to_string()]
    }

    fn workspace_configs(&self) -> &[&str] {
        &["go.work"]
    }
}
