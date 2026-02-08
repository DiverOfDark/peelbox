//! npm build system (JavaScript/TypeScript)

use super::node_common::{parse_node_version, read_node_version_file};
use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::language::LanguageDefinition;
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

pub struct NpmBuildSystem;

impl BuildSystem for NpmBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Npm
    }

    fn language_id(&self) -> Option<crate::LanguageId> {
        Some(crate::LanguageId::JavaScript)
    }

    fn runtime_id(&self) -> Option<crate::RuntimeId> {
        Some(crate::RuntimeId::Node)
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "package.json".to_string(),
            priority: 10,
        }]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for rel_path in file_tree {
            let filename = rel_path.file_name().and_then(|n| n.to_str());

            let is_match = match filename {
                Some("package.json") => {
                    let abs_path = repo_root.join(rel_path);
                    let content = fs.read_to_string(&abs_path).ok();
                    if let Some(c) = content.as_deref() {
                        let is_npm = !c.contains("\"packageManager\": \"pnpm")
                            && !c.contains("\"packageManager\": \"yarn")
                            && !c.contains("\"packageManager\": \"bun");

                        if is_npm {
                            let lang = crate::language::JavaScriptLanguage;
                            let project_dir = rel_path.parent().unwrap_or(Path::new(""));

                            lang.is_runnable(
                                fs,
                                repo_root,
                                project_dir,
                                file_tree,
                                content.as_deref(),
                            )
                        } else {
                            false
                        }
                    } else {
                        // Can't read file, but filename matches - optimistically assume yes but is_runnable would fail anyway without content
                        // Actually, if we can't read content, is_runnable might still work with other files
                        // But for now let's keep previous behavior of falling back to true, but guarded by is_runnable
                        let lang = crate::language::JavaScriptLanguage;
                        let project_dir = rel_path.parent().unwrap_or(Path::new(""));
                        lang.is_runnable(fs, repo_root, project_dir, file_tree, None)
                    }
                }
                _ => false,
            };

            if is_match {
                detections.push(DetectionStack::new(
                    BuildSystemId::Npm,
                    LanguageId::JavaScript,
                    rel_path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        service_path: &Path,
        relative_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let node_version = read_node_version_file(service_path)
            .or_else(|| manifest_content.and_then(parse_node_version))
            .or_else(|| wolfi_index.get_latest_version("nodejs"))
            .expect("Failed to get nodejs version from Wolfi index");

        let build_env = std::collections::BTreeMap::new();

        let is_root = relative_path.components().count() == 0 || relative_path == Path::new(".");
        let service_dir = relative_path.to_string_lossy();

        // Check if there's a root package.json (workspace project) for non-root services.
        // Navigate from service_path to the repo root by going up relative_path's depth.
        let has_root_package_json = !is_root && {
            let mut root = service_path.to_path_buf();
            for _ in relative_path.components() {
                root = root.parent().unwrap_or(Path::new("")).to_path_buf();
            }
            let root_pkg = root.join("package.json");
            if root_pkg.exists() {
                std::fs::read_to_string(&root_pkg)
                    .map(|content| self.is_workspace_root(Some(&content)))
                    .unwrap_or(false)
            } else {
                false
            }
        };

        let mut build_commands = if is_root || has_root_package_json {
            // Root project or workspace: npm ci at root
            vec!["npm ci".to_string()]
        } else {
            // Standalone project in a subdirectory: npm ci within the service dir
            vec![format!("cd {} && npm ci", service_dir)]
        };
        if is_root {
            build_commands.push("npm run build".to_string());
        } else {
            build_commands.push(format!("cd {} && npm run build", service_dir));
        }

        // For Node.js workspaces, always copy the entire root context
        // because npm hoists dependencies to the root node_modules
        let runtime_copy = vec![(".".to_string(), "/app/".to_string())];

        BuildTemplate {
            build_packages: vec![node_version, "npm".to_string()],
            build_commands,
            cache_paths: vec!["/root/.npm/".to_string()],
            common_ports: vec![3000, 8080],
            build_env,
            runtime_copy,
            runtime_env: std::collections::BTreeMap::new(),
            runtime_workdir: None,
            entrypoint: None,
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
        vec![
            "lerna.json".to_string(),
            "nx.json".to_string(),
            "turbo.json".to_string(),
            "rush.json".to_string(),
        ]
    }

    fn metadata_manifest_file(&self) -> Option<&str> {
        Some("package.json")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const MANIFEST_WITH_ENGINE: &str = r#"{"name": "app", "version": "1.0.0", "engines": {"node": "22"}, "scripts": {"start": "node index.js"}}"#;

    #[test]
    fn test_root_package_json_without_workspaces_not_treated_as_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Root package.json without "workspaces" field
        fs::write(
            root.join("package.json"),
            r#"{"name": "root", "version": "1.0.0"}"#,
        )
        .unwrap();

        // Sub-project
        let sub = root.join("packages").join("app");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("package.json"), MANIFEST_WITH_ENGINE).unwrap();

        let npm = NpmBuildSystem;
        let wolfi = peelbox_wolfi::WolfiPackageIndex::fetch().unwrap();
        let relative = Path::new("packages/app");
        let template = npm.build_template(&wolfi, &sub, relative, Some(MANIFEST_WITH_ENGINE));

        // Without workspaces, the sub-project should install deps in its own dir
        assert!(
            template.build_commands[0].contains("cd packages/app"),
            "Expected standalone install, got: {:?}",
            template.build_commands
        );
    }

    #[test]
    fn test_root_package_json_with_workspaces_treated_as_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Root package.json WITH "workspaces" field
        fs::write(
            root.join("package.json"),
            r#"{"name": "root", "version": "1.0.0", "workspaces": ["packages/*"]}"#,
        )
        .unwrap();

        // Sub-project
        let sub = root.join("packages").join("app");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("package.json"), MANIFEST_WITH_ENGINE).unwrap();

        let npm = NpmBuildSystem;
        let wolfi = peelbox_wolfi::WolfiPackageIndex::fetch().unwrap();
        let relative = Path::new("packages/app");
        let template = npm.build_template(&wolfi, &sub, relative, Some(MANIFEST_WITH_ENGINE));

        // With workspaces, npm ci at root
        assert_eq!(
            template.build_commands[0], "npm ci",
            "Expected workspace install at root, got: {:?}",
            template.build_commands
        );
    }
}
