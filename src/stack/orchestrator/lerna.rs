//! Lerna orchestrator

use super::{MonorepoOrchestrator, OrchestratorId, Package, WorkspaceStructure};
use crate::stack::buildsystem::{BuildSystem, NpmBuildSystem};
use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

pub struct LernaOrchestrator;

impl MonorepoOrchestrator for LernaOrchestrator {
    fn id(&self) -> OrchestratorId {
        OrchestratorId::Lerna
    }

    fn config_files(&self) -> &[&str] {
        &["lerna.json"]
    }

    fn detect(&self, config_file: &str, _content: Option<&str>) -> bool {
        config_file == "lerna.json"
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["node_modules".to_string()]
    }

    fn name(&self) -> &'static str {
        "Lerna"
    }

    fn workspace_structure(&self, repo_path: &Path) -> Result<WorkspaceStructure> {
        parse_workspace_structure(repo_path)
    }

    fn build_command(&self, package: &Package) -> String {
        format!("lerna run build --scope={}", package.name)
    }
}

fn parse_workspace_structure(repo_path: &Path) -> Result<WorkspaceStructure> {
    let lerna_json_path = repo_path.join("lerna.json");
    let lerna_content = std::fs::read_to_string(&lerna_json_path)
        .with_context(|| format!("Failed to read {}", lerna_json_path.display()))?;

    let lerna_config: Value = serde_json::from_str(&lerna_content)
        .with_context(|| "Failed to parse lerna.json")?;

    let npm = NpmBuildSystem;
    let mut packages = Vec::new();

    let patterns = if let Some(workspace_patterns) = lerna_config["packages"].as_array() {
        workspace_patterns
            .iter()
            .filter_map(|v| v.as_str())
            .map(String::from)
            .collect()
    } else {
        vec!["packages/*".to_string()]
    };

    for pattern in patterns {
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
        orchestrator: Some(OrchestratorId::Lerna),
        packages,
    })
}

