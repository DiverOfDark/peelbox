//! npm build system (JavaScript/TypeScript)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct NpmBuildSystem;

impl BuildSystem for NpmBuildSystem {
    fn name(&self) -> &str {
        "npm"
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "package.json",
                priority: 10,
            },
            ManifestPattern {
                filename: "package-lock.json",
                priority: 12,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        match manifest_name {
            "package-lock.json" => true,
            "package.json" => {
                if let Some(content) = manifest_content {
                    !content.contains("\"packageManager\": \"pnpm")
                        && !content.contains("\"packageManager\": \"yarn")
                        && !content.contains("\"packageManager\": \"bun")
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "node:20".to_string(),
            runtime_image: "node:20-slim".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: vec!["npm ci".to_string(), "npm run build".to_string()],
            cache_paths: vec!["node_modules/".to_string(), ".npm/".to_string()],
            artifacts: vec!["dist/".to_string(), "build/".to_string()],
            common_ports: vec![3000, 8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".npm".to_string()]
    }

    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("\"workspaces\"")
        } else {
            false
        }
    }

    fn workspace_configs(&self) -> &[&str] {
        &["lerna.json", "nx.json", "turbo.json", "rush.json"]
    }
}
