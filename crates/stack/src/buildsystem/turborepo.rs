//! Turborepo build system (JavaScript/TypeScript monorepos)

use super::node_common::{parse_node_version, read_node_version_file};
use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

pub struct TurborepoBuildSystem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
}

fn detect_package_manager(service_path: &Path) -> PackageManager {
    // 1. Check lock files
    if service_path.join("pnpm-lock.yaml").exists() {
        return PackageManager::Pnpm;
    }
    if service_path.join("yarn.lock").exists() {
        return PackageManager::Yarn;
    }

    // 2. Check packageManager field in package.json
    if let Ok(content) = std::fs::read_to_string(service_path.join("package.json")) {
        if content.contains("\"packageManager\": \"pnpm") {
            return PackageManager::Pnpm;
        }
        if content.contains("\"packageManager\": \"yarn") {
            return PackageManager::Yarn;
        }
    }

    // 3. Default to npm
    PackageManager::Npm
}

impl BuildSystem for TurborepoBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Turborepo
    }

    fn language_id(&self) -> Option<crate::LanguageId> {
        Some(crate::LanguageId::JavaScript)
    }

    fn runtime_id(&self) -> Option<crate::RuntimeId> {
        Some(crate::RuntimeId::Node)
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "turbo.json".to_string(),
            priority: 20,
        }]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for path in file_tree {
            if path.file_name().and_then(|n| n.to_str()) == Some("turbo.json") {
                // Verify package.json exists in the same directory
                let dir = path.parent().unwrap_or(Path::new(""));
                let package_json_path = if dir.as_os_str().is_empty() {
                    PathBuf::from("package.json")
                } else {
                    dir.join("package.json")
                };

                let has_package_json = file_tree.iter().any(|p| p == &package_json_path)
                    || repo_root.join(&package_json_path).exists();

                if has_package_json {
                    detections.push(DetectionStack::new(
                        BuildSystemId::Turborepo,
                        LanguageId::JavaScript,
                        path.clone(),
                    ));
                }
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        service_path: &Path,
        _relative_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let pkg_manager = detect_package_manager(service_path);

        // Node.js version: read from .nvmrc/.node-version, then package.json engines, then wolfi index
        let package_json_content = std::fs::read_to_string(service_path.join("package.json")).ok();

        let node_version = read_node_version_file(service_path)
            .or_else(|| package_json_content.as_deref().and_then(parse_node_version))
            .or_else(|| wolfi_index.get_latest_version("nodejs"))
            .expect("Failed to get nodejs version from Wolfi index");

        let (pkg_manager_name, install_cmd, turbo_cmd) = match pkg_manager {
            PackageManager::Npm => ("npm", "npm install", "npx turbo build"),
            PackageManager::Pnpm => ("pnpm", "pnpm install", "pnpm turbo build"),
            PackageManager::Yarn => ("yarn", "yarn install", "yarn turbo build"),
        };

        let mut build_packages = vec![
            node_version,
            pkg_manager_name.to_string(),
            "build-base".to_string(),
            "python-3.12".to_string(),
            "npm".to_string(),
            "ca-certificates".to_string(),
            "git".to_string(),
        ];
        // Deduplicate if pkg_manager is npm (already in the list)
        if pkg_manager == PackageManager::Npm {
            build_packages.dedup();
        }

        let mut cache_paths = vec!["node_modules/".to_string(), ".turbo/".to_string()];
        match pkg_manager {
            PackageManager::Pnpm => cache_paths.push(".pnpm-store/".to_string()),
            PackageManager::Yarn => cache_paths.push(".yarn/cache/".to_string()),
            PackageManager::Npm => {}
        }

        BuildTemplate {
            build_packages,
            build_commands: vec![install_cmd.to_string(), turbo_cmd.to_string()],
            cache_paths,
            common_ports: vec![3000, 8080],
            build_env: std::collections::BTreeMap::new(),
            runtime_copy: vec![(".".to_string(), "/app/".to_string())],
            runtime_env: std::collections::BTreeMap::new(),
            runtime_workdir: None,
            entrypoint: None,
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".turbo".to_string()]
    }

    fn is_workspace_root(&self, _manifest_content: Option<&str>) -> bool {
        true
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec!["turbo.json".to_string()]
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
