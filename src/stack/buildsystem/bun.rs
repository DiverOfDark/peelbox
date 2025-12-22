//! Bun build system (JavaScript/TypeScript)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct BunBuildSystem;

impl BuildSystem for BunBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Bun
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "bun.lockb".to_string(),
                priority: 15,
            },
            ManifestPattern {
                filename: "package.json".to_string(),
                priority: 10,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        match manifest_name {
            "bun.lockb" => true,
            "package.json" => {
                if let Some(content) = manifest_content {
                    content.contains("\"packageManager\": \"bun")
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "oven/bun:1".to_string(),
            runtime_image: "oven/bun:1-slim".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec!["bun install".to_string(), "bun run build".to_string()],
            cache_paths: vec!["node_modules/".to_string(), ".bun/".to_string()],
            artifacts: vec!["dist/".to_string(), "build/".to_string()],
            common_ports: vec![3000, 8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".bun".to_string()]
    }

    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("\"workspaces\"")
        } else {
            false
        }
    }
}
