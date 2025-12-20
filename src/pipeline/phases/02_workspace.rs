use super::scan::ScanResult;
use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use crate::stack::buildsystem::BuildSystem;
use crate::stack::orchestrator::{OrchestratorId, Package, WorkspaceStructure};
use crate::stack::StackRegistry;
use anyhow::Result;
use async_trait::async_trait;

pub struct WorkspaceStructurePhase;

#[async_trait]
impl WorkflowPhase for WorkspaceStructurePhase {
    fn name(&self) -> &'static str {
        "WorkspaceStructurePhase"
    }

    fn try_deterministic(&self, context: &mut AnalysisContext) -> Result<Option<()>> {
        let scan = context
            .scan
            .as_ref()
            .expect("Scan must be available before WorkspaceStructurePhase");

        let workspace_structure = detect_workspace_structure(&context.repo_path, scan, &context.stack_registry)?;
        context.workspace = Some(workspace_structure);
        Ok(Some(()))
    }

    async fn execute_llm(&self, context: &mut AnalysisContext) -> Result<()> {
        let scan = context
            .scan
            .as_ref()
            .expect("Scan must be available before WorkspaceStructurePhase");

        let workspace_structure = detect_workspace_structure(&context.repo_path, scan, &context.stack_registry)?;
        context.workspace = Some(workspace_structure);
        Ok(())
    }
}

fn detect_workspace_structure(
    repo_path: &std::path::Path,
    scan: &ScanResult,
    stack_registry: &StackRegistry,
) -> Result<WorkspaceStructure> {
    // Try to detect orchestrator from scan results using StackRegistry
    for orchestrator in stack_registry.all_orchestrators() {
        for config_file in orchestrator.config_files() {
            // Check if orchestrator config file exists in file tree
            if scan.find_files_by_name(config_file).iter().any(|f| {
                f.parent().unwrap_or(repo_path) == repo_path
            }) {
                // Found orchestrator config file at root, try to parse workspace structure
                if let Ok(structure) = orchestrator.workspace_structure(repo_path) {
                    return Ok(structure);
                }
            }
        }
    }

    // No orchestrator found - create single-service workspace
    let first_detection = scan
        .detections
        .first()
        .ok_or_else(|| anyhow::anyhow!("No detections found in scan"))?;

    let service_path = first_detection.manifest_path
        .parent()
        .unwrap_or(repo_path)
        .to_path_buf();

    // Extract package name from manifest using build system
    let (name, is_application) = if let Some(build_system) = stack_registry.get_build_system(first_detection.build_system) {
        let manifest_path = repo_path.join(&first_detection.manifest_path);
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            build_system
                .parse_package_metadata(&content)
                .unwrap_or_else(|_| ("app".to_string(), true))
        } else {
            ("app".to_string(), true)
        }
    } else {
        ("app".to_string(), true)
    };

    Ok(WorkspaceStructure {
        orchestrator: OrchestratorId::Turborepo, // Placeholder - single service has no orchestrator
        packages: vec![Package {
            path: service_path,
            name,
            is_application,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::BuildSystemId;
    use crate::stack::LanguageId;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn test_single_service_workspace() {
        use super::super::scan::RepoSummary;
        use crate::stack::DetectionStack;

        let scan = ScanResult {
            repo_path: PathBuf::from("."),
            summary: RepoSummary {
                manifest_count: 1,
                primary_language: Some("JavaScript".to_string()),
                primary_build_system: Some("npm".to_string()),
                is_monorepo: false,
                root_manifests: vec!["package.json".to_string()],
            },
            detections: vec![DetectionStack::new(
                BuildSystemId::Npm,
                LanguageId::JavaScript,
                PathBuf::from("package.json"),
            )],
            workspace: super::super::scan::WorkspaceInfo {
                root_manifests: vec!["package.json".to_string()],
                nested_by_depth: HashMap::new(),
                max_depth: 0,
                has_workspace_config: false,
            },
            file_tree: vec![PathBuf::from("package.json")],
            scan_time_ms: 0,
        };

        let registry = StackRegistry::with_defaults();
        let workspace = detect_workspace_structure(&PathBuf::from("."), &scan, &registry).unwrap();
        assert_eq!(workspace.packages.len(), 1);
        assert_eq!(workspace.packages[0].name, "app");
        assert!(workspace.packages[0].is_application);
    }
}
