//! Turborepo orchestrator (Vercel)

use super::{MonorepoOrchestrator, OrchestratorId, Package, WorkspaceStructure};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

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

    let root_package: Value = serde_json::from_str(&content)
        .with_context(|| "Failed to parse root package.json")?;

    let workspaces = root_package["workspaces"]
        .as_array()
        .context("Missing or invalid workspaces field")?;

    let mut packages = Vec::new();

    for workspace_pattern in workspaces {
        let pattern = workspace_pattern
            .as_str()
            .context("Workspace pattern must be string")?;

        let workspace_paths = glob_workspace(repo_path, pattern)?;

        for workspace_path in workspace_paths {
            if let Ok(pkg) = parse_package(&workspace_path) {
                packages.push(pkg);
            }
        }
    }

    Ok(WorkspaceStructure {
        orchestrator: OrchestratorId::Turborepo,
        packages,
    })
}

fn parse_package(workspace_path: &Path) -> Result<Package> {
    let package_json_path = workspace_path.join("package.json");
    let content = std::fs::read_to_string(&package_json_path)?;
    let package: Value = serde_json::from_str(&content)?;

    let name = package["name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    // Package is an application if it has a "start" script
    let is_application = package["scripts"]["start"].is_string();

    Ok(Package {
        path: workspace_path.to_path_buf(),
        name,
        is_application,
    })
}

fn glob_workspace(repo_path: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
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
