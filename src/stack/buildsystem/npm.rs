//! npm build system (JavaScript/TypeScript)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct NpmBuildSystem;

impl BuildSystem for NpmBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::Npm
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "package.json".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "package-lock.json".to_string(),
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

    fn workspace_configs(&self) -> Vec<String> {
        vec!["lerna.json".to_string(), "nx.json".to_string(), "turbo.json".to_string(), "rush.json".to_string()]
    }

    fn parse_package_metadata(
        &self,
        manifest_content: &str,
    ) -> Result<(String, bool), anyhow::Error> {
        let package: serde_json::Value = serde_json::from_str(manifest_content)?;

        let name = package["name"].as_str().unwrap_or("unknown").to_string();

        let is_application = package["scripts"]["start"].is_string();

        Ok((name, is_application))
    }

    fn parse_workspace_patterns(
        &self,
        manifest_content: &str,
    ) -> Result<Vec<String>, anyhow::Error> {
        super::parse_package_json_workspaces(manifest_content)
    }

    fn glob_workspace_pattern(
        &self,
        repo_path: &std::path::Path,
        pattern: &str,
    ) -> Result<Vec<std::path::PathBuf>, anyhow::Error> {
        super::glob_package_json_workspace_pattern(repo_path, pattern)
    }
}
