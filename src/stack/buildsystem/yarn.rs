//! Yarn build system (JavaScript/TypeScript)

use super::{BuildSystem, BuildTemplate, ManifestPattern, WorkspaceBuildSystem};
use anyhow::Result;

pub struct YarnBuildSystem;

impl BuildSystem for YarnBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Yarn
    }

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "yarn.lock",
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
            "yarn.lock" => true,
            "package.json" => {
                if let Some(content) = manifest_content {
                    content.contains("\"packageManager\": \"yarn")
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
                "yarn install --frozen-lockfile".to_string(),
                "yarn build".to_string(),
            ],
            cache_paths: vec!["node_modules/".to_string(), ".yarn/cache/".to_string()],
            artifacts: vec!["dist/".to_string(), "build/".to_string()],
            common_ports: vec![3000, 8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".yarn".to_string()]
    }
    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            content.contains("\"workspaces\"")
        } else {
            false
        }
    }

    fn workspace_configs(&self) -> &[&str] {
        &["lerna.json", "nx.json", "turbo.json"]
    }
}

impl WorkspaceBuildSystem for YarnBuildSystem {
    fn parse_workspace_patterns(&self, manifest_content: &str) -> Result<Vec<String>> {
        let package: serde_json::Value = serde_json::from_str(manifest_content)?;

        if let Some(workspaces) = package["workspaces"].as_array() {
            Ok(workspaces
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect())
        } else {
            Ok(vec![])
        }
    }

    fn glob_workspace_pattern(&self, repo_path: &std::path::Path, pattern: &str) -> Result<Vec<std::path::PathBuf>> {
        let mut results = Vec::new();

        if pattern.ends_with("/*") {
            let base_dir = repo_path.join(pattern.trim_end_matches("/*"));
            if let Ok(entries) = std::fs::read_dir(&base_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        results.push(entry.path());
                    }
                }
            }
        }

        Ok(results)
    }
}
