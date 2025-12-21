//! pnpm build system (JavaScript/TypeScript)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use anyhow::Result;

pub struct PnpmBuildSystem;

impl BuildSystem for PnpmBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Pnpm
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "pnpm-lock.yaml",
                priority: 15,
            },
            ManifestPattern {
                filename: "package.json",
                priority: 10,
            },
        ]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        match manifest_name {
            "pnpm-lock.yaml" => true,
            "package.json" => {
                if let Some(content) = manifest_content {
                    content.contains("\"packageManager\": \"pnpm")
                } else {
                    false
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
            build_commands: vec![
                "corepack enable".to_string(),
                "pnpm install --frozen-lockfile".to_string(),
                "pnpm build".to_string(),
            ],
            cache_paths: vec!["node_modules/".to_string(), ".pnpm-store/".to_string()],
            artifacts: vec!["dist/".to_string(), "build/".to_string()],
            common_ports: vec![3000, 8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".pnpm-store".to_string()]
    }
    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("\"workspaces\"")
        } else {
            false
        }
    }

    fn workspace_configs(&self) -> &[&str] {
        &["pnpm-workspace.yaml", "turbo.json"]
    }

    fn parse_workspace_patterns(&self, manifest_content: &str) -> Result<Vec<String>> {
        super::parse_package_json_workspaces(manifest_content)
    }

    fn glob_workspace_pattern(&self, repo_path: &std::path::Path, pattern: &str) -> Result<Vec<std::path::PathBuf>> {
        super::glob_package_json_workspace_pattern(repo_path, pattern)
    }
}
