//! Nx orchestrator (Nrwl)

use super::{MonorepoOrchestrator, OrchestratorId, Package, WorkspaceStructure};
use crate::buildsystem::{BuildSystem, NpmBuildSystem};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

pub struct NxOrchestrator;

impl MonorepoOrchestrator for NxOrchestrator {
    fn id(&self) -> OrchestratorId {
        OrchestratorId::Nx
    }

    fn config_files(&self) -> Vec<String> {
        vec!["nx.json".to_string()]
    }

    fn detect(&self, config_file: &str, _content: Option<&str>) -> bool {
        config_file == "nx.json"
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string(), ".nx".to_string()]
    }

    fn name(&self) -> &'static str {
        "Nx"
    }

    fn workspace_structure(&self, repo_path: &Path) -> Result<WorkspaceStructure> {
        parse_workspace_structure(repo_path)
    }

    fn build_command(&self, package: &Package) -> String {
        format!("nx build {}", package.name)
    }
}

fn parse_workspace_structure(repo_path: &Path) -> Result<WorkspaceStructure> {
    let nx_json_path = repo_path.join("nx.json");
    let _nx_content = std::fs::read_to_string(&nx_json_path)
        .with_context(|| format!("Failed to read {}", nx_json_path.display()))?;

    let npm = NpmBuildSystem;
    let mut packages = Vec::new();

    // Try workspace.json first (Nx >= 13)
    let workspace_json_path = repo_path.join("workspace.json");
    if workspace_json_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&workspace_json_path) {
            if let Ok(workspace) = serde_json::from_str::<Value>(&content) {
                if let Some(projects) = workspace["projects"].as_object() {
                    for (name, project_config) in projects {
                        if let Some(root) = project_config.as_str() {
                            let project_path = repo_path.join(root);
                            if let Ok(pkg) = parse_project(&project_path, name, &npm) {
                                packages.push(pkg);
                            }
                        }
                    }
                }
            }
        }
    }

    // Try package.json workspaces (Nx < 13 or npm workspaces integration)
    if packages.is_empty() {
        let package_json_path = repo_path.join("package.json");
        if package_json_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&package_json_path) {
                let patterns = npm.parse_workspace_patterns(&content).unwrap_or_default();
                for pattern in patterns {
                    if let Ok(workspace_paths) = npm.glob_workspace_pattern(repo_path, &pattern) {
                        for workspace_path in workspace_paths {
                            let pkg_json = workspace_path.join("package.json");
                            if let Ok(pkg_content) = std::fs::read_to_string(&pkg_json) {
                                if let Ok((name, is_application)) =
                                    npm.parse_package_metadata(&pkg_content)
                                {
                                    packages.push(Package {
                                        path: workspace_path,
                                        name,
                                        is_application,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(WorkspaceStructure {
        orchestrator: Some(OrchestratorId::Nx),
        packages,
    })
}

fn parse_project(project_path: &Path, name: &str, npm: &NpmBuildSystem) -> Result<Package> {
    // Check project.json first (Nx >= 13 project configuration)
    let project_json_path = project_path.join("project.json");
    let is_application = if project_json_path.exists() {
        let content = std::fs::read_to_string(&project_json_path)?;
        let project: Value = serde_json::from_str(&content)?;
        // Nx applications have "serve" or "start" targets
        project["targets"]["serve"].is_object() || project["targets"]["start"].is_object()
    } else {
        // Fallback to package.json analysis via build system
        let package_json_path = project_path.join("package.json");
        if package_json_path.exists() {
            let content = std::fs::read_to_string(&package_json_path)?;
            // Use npm build system to detect application (checks for "start" script)
            npm.parse_package_metadata(&content)?.1
        } else {
            false
        }
    };

    Ok(Package {
        path: project_path.to_path_buf(),
        name: name.to_string(),
        is_application,
    })
}
