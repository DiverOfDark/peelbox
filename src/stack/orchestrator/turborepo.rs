//! Turborepo orchestrator (Vercel)

use super::{MonorepoOrchestrator, OrchestratorId, Package, WorkspaceStructure};
use crate::stack::buildsystem::{BuildSystem, NpmBuildSystem, WorkspaceBuildSystem};
use anyhow::{Context, Result};
use std::path::Path;

pub struct TurborepoOrchestrator;

impl MonorepoOrchestrator for TurborepoOrchestrator {
    fn id(&self) -> OrchestratorId {
        OrchestratorId::Turborepo
    }

    fn config_files(&self) -> &[&str] {
        &["turbo.json"]
    }

    fn detect(&self, config_file: &str, _content: Option<&str>) -> bool {
        config_file == "turbo.json"
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".turbo".to_string()]
    }

    fn name(&self) -> &'static str {
        "Turborepo"
    }

    fn workspace_structure(&self, repo_path: &Path) -> Result<WorkspaceStructure> {
        parse_workspace_structure(repo_path)
    }

    fn build_command(&self, package: &Package) -> String {
        format!("turbo run build --filter={}", package.name)
    }
}

fn parse_workspace_structure(repo_path: &Path) -> Result<WorkspaceStructure> {
    let package_json_path = repo_path.join("package.json");
    let content = std::fs::read_to_string(&package_json_path)
        .with_context(|| format!("Failed to read {}", package_json_path.display()))?;

    let npm = NpmBuildSystem;
    let workspace_patterns = npm.parse_workspace_patterns(&content)
        .context("Failed to parse workspace patterns")?;

    let mut packages = Vec::new();

    for pattern in workspace_patterns {
        let workspace_paths = npm.glob_workspace_pattern(repo_path, &pattern)?;

        for workspace_path in workspace_paths {
            let pkg_json = workspace_path.join("package.json");
            if let Ok(pkg_content) = std::fs::read_to_string(&pkg_json) {
                if let Ok((name, is_application)) = npm.parse_package_metadata(&pkg_content) {
                    packages.push(Package {
                        path: workspace_path,
                        name,
                        is_application,
                    });
                }
            }
        }
    }

    Ok(WorkspaceStructure {
        orchestrator: Some(OrchestratorId::Turborepo),
        packages,
    })
}

