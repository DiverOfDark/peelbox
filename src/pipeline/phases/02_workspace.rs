use super::scan::ScanResult;
use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use crate::stack::orchestrator::{Package, WorkspaceStructure};
use crate::stack::StackRegistry;
use anyhow::Result;
use async_trait::async_trait;

pub struct WorkspaceStructurePhase;

#[async_trait]
impl WorkflowPhase for WorkspaceStructurePhase {
    fn name(&self) -> &'static str {
        "WorkspaceStructurePhase"
    }

    async fn execute(&self, context: &mut AnalysisContext) -> Result<()> {
        let scan = context
            .scan
            .as_ref()
            .expect("Scan must be available before WorkspaceStructurePhase");

        let workspace_structure =
            detect_workspace_structure(&context.repo_path, scan, &context.stack_registry)?;
        context.workspace = Some(workspace_structure);
        Ok(())
    }
}

fn extract_package_metadata(
    detection: &crate::stack::DetectionStack,
    repo_path: &std::path::Path,
    stack_registry: &StackRegistry,
) -> (String, bool) {
    stack_registry
        .get_build_system(detection.build_system.clone())
        .and_then(|bs| {
            let manifest_path = repo_path.join(&detection.manifest_path);
            std::fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|content| bs.parse_package_metadata(&content).ok())
        })
        .unwrap_or_else(|| ("app".to_string(), true))
}

fn is_workspace_root_manifest(
    detection: &crate::stack::DetectionStack,
    repo_path: &std::path::Path,
    stack_registry: &StackRegistry,
) -> bool {
    // Check if manifest is at repo root (parent is empty path)
    let parent = detection
        .manifest_path
        .parent()
        .unwrap_or(std::path::Path::new(""));
    if parent != std::path::Path::new("") {
        return false;
    }

    let manifest_path = repo_path.join(&detection.manifest_path);
    let Ok(content) = std::fs::read_to_string(&manifest_path) else {
        return false;
    };

    stack_registry
        .get_build_system(detection.build_system.clone())
        .map(|bs| bs.is_workspace_root(Some(&content)))
        .unwrap_or(false)
}

fn create_package(
    detection: &crate::stack::DetectionStack,
    repo_path: &std::path::Path,
    stack_registry: &StackRegistry,
) -> Package {
    let service_path = detection
        .manifest_path
        .parent()
        .unwrap_or(repo_path)
        .to_path_buf();

    let (name, is_application) = extract_package_metadata(detection, repo_path, stack_registry);

    Package {
        path: service_path,
        name,
        is_application,
    }
}

fn try_workspace_build_system(
    detection: &crate::stack::DetectionStack,
    repo_path: &std::path::Path,
    stack_registry: &StackRegistry,
) -> Result<Option<WorkspaceStructure>> {
    let manifest_path = repo_path.join(&detection.manifest_path);
    let Ok(manifest_content) = std::fs::read_to_string(&manifest_path) else {
        return Ok(None);
    };

    let Some(build_system) = stack_registry.get_build_system(detection.build_system.clone()) else {
        return Ok(None);
    };

    let workspace_patterns = build_system.parse_workspace_patterns(&manifest_content)?;

    if workspace_patterns.is_empty() {
        return Ok(None);
    }

    let mut packages = Vec::new();

    for pattern in workspace_patterns {
        let paths = build_system.glob_workspace_pattern(repo_path, &pattern)?;

        for package_path in paths {
            let name = package_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let relative_path = package_path
                .strip_prefix(repo_path)
                .unwrap_or(&package_path)
                .to_path_buf();

            packages.push(Package {
                path: relative_path,
                name,
                is_application: true,
            });
        }
    }

    if packages.is_empty() {
        Ok(None)
    } else {
        Ok(Some(WorkspaceStructure {
            orchestrator: None,
            packages,
        }))
    }
}

fn detect_workspace_structure(
    repo_path: &std::path::Path,
    scan: &ScanResult,
    stack_registry: &StackRegistry,
) -> Result<WorkspaceStructure> {
    for orchestrator in stack_registry.all_orchestrators() {
        for config_file in orchestrator.config_files() {
            if scan
                .find_files_by_name(&config_file)
                .iter()
                .any(|f| f.parent().unwrap_or(repo_path) == repo_path)
            {
                if let Ok(structure) = orchestrator.workspace_structure(repo_path) {
                    return Ok(structure);
                }
            }
        }
    }

    for detection in &scan.detections {
        if is_workspace_root_manifest(detection, repo_path, stack_registry) {
            if let Some(mut workspace_structure) =
                try_workspace_build_system(detection, repo_path, stack_registry)?
            {
                // Also include standalone modules not in workspace
                let workspace_paths: std::collections::HashSet<_> = workspace_structure
                    .packages
                    .iter()
                    .map(|p| p.path.clone())
                    .collect();

                let standalone_packages: Vec<Package> = scan
                    .detections
                    .iter()
                    .filter(|d| !is_workspace_root_manifest(d, repo_path, stack_registry))
                    .map(|d| create_package(d, repo_path, stack_registry))
                    .filter(|p| !workspace_paths.contains(&p.path))
                    .collect();

                workspace_structure.packages.extend(standalone_packages);
                return Ok(workspace_structure);
            }
        }
    }

    let packages: Vec<Package> = scan
        .detections
        .iter()
        .filter(|d| !is_workspace_root_manifest(d, repo_path, stack_registry))
        .map(|d| create_package(d, repo_path, stack_registry))
        .collect();

    if packages.is_empty() && !scan.detections.is_empty() {
        let package = create_package(&scan.detections[0], repo_path, stack_registry);
        return Ok(WorkspaceStructure {
            orchestrator: None,
            packages: vec![package],
        });
    }

    Ok(WorkspaceStructure {
        orchestrator: None,
        packages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::BuildSystemId;
    use crate::stack::LanguageId;
    use std::collections::BTreeMap;
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
                nested_by_depth: BTreeMap::new(),
                max_depth: 0,
                has_workspace_config: false,
            },
            file_tree: vec![PathBuf::from("package.json")],
            scan_time_ms: 0,
        };

        let registry = StackRegistry::with_defaults(None);
        let workspace = detect_workspace_structure(&PathBuf::from("."), &scan, &registry).unwrap();
        assert_eq!(workspace.packages.len(), 1);
        assert_eq!(workspace.packages[0].name, "app");
        assert!(workspace.packages[0].is_application);
    }
}
