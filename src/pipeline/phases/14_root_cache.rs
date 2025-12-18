use super::scan::ScanResult;
use super::structure::StructureResult;
use crate::pipeline::Confidence;
use crate::stack::registry::StackRegistry;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCacheInfo {
    pub root_cache_dirs: Vec<PathBuf>,
    pub confidence: Confidence,
}

pub fn execute(scan: &ScanResult, structure: &StructureResult) -> RootCacheInfo {
    let mut root_cache_dirs = HashSet::new();

    let registry = StackRegistry::with_defaults();

    // Add cache dirs from workspace root build systems
    for detection in &scan.detections {
        if detection.is_workspace_root {
            if let Some(build_system) = registry.get_build_system(detection.build_system) {
                for cache_dir in build_system.cache_dirs() {
                    root_cache_dirs.insert(PathBuf::from(cache_dir));
                }
            }
        }
    }

    // Add cache dirs from orchestrator (if detected)
    if let Some(orchestrator_name) = &structure.orchestrator {
        for orchestrator in registry.all_orchestrators() {
            if orchestrator.name().to_lowercase() == orchestrator_name.to_lowercase() {
                for cache_dir in orchestrator.cache_dirs() {
                    root_cache_dirs.insert(PathBuf::from(cache_dir));
                }
                break;
            }
        }
    }

    let mut dirs: Vec<PathBuf> = root_cache_dirs.into_iter().collect();
    dirs.sort();

    RootCacheInfo {
        root_cache_dirs: dirs,
        confidence: Confidence::High,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::phases::scan::{RepoSummary, WorkspaceInfo};
    use crate::pipeline::phases::structure::ProjectType;
    use crate::pipeline::Confidence;
    use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
    use std::collections::HashMap;

    #[test]
    fn test_root_cache_pnpm() {
        let mut scan = create_scan_with_files(vec!["pnpm-workspace.yaml"]);
        scan.detections[0].build_system = BuildSystemId::Pnpm;
        scan.detections[0].is_workspace_root = true;

        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            services: vec![],
            packages: vec![],
            orchestrator: None,
            confidence: Confidence::High,
        };

        let result = execute(&scan, &structure);
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from(".pnpm-store")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_root_cache_cargo_workspace() {
        let mut scan = create_scan_with_files(vec!["Cargo.toml"]);
        scan.detections[0].is_workspace_root = true;

        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            services: vec![],
            packages: vec![],
            orchestrator: None,
            confidence: Confidence::High,
        };

        let result = execute(&scan, &structure);
        assert!(result.root_cache_dirs.contains(&PathBuf::from("target")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".cargo")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_root_cache_turborepo() {
        let scan = create_scan_with_files(vec!["turbo.json", "package.json"]);

        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            services: vec![],
            packages: vec![],
            orchestrator: Some("turborepo".to_string()),
            confidence: Confidence::High,
        };

        let result = execute(&scan, &structure);
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".turbo")));
    }

    #[test]
    fn test_root_cache_none() {
        let scan = create_scan_with_files(vec!["package.json"]);

        let structure = StructureResult {
            project_type: ProjectType::SingleService,
            services: vec![],
            packages: vec![],
            orchestrator: None,
            confidence: Confidence::High,
        };

        let result = execute(&scan, &structure);
        assert!(result.root_cache_dirs.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_root_cache_nx() {
        let scan = create_scan_with_files(vec!["nx.json", "package.json"]);

        let structure = StructureResult {
            project_type: ProjectType::Monorepo,
            services: vec![],
            packages: vec![],
            orchestrator: Some("nx".to_string()),
            confidence: Confidence::High,
        };

        let result = execute(&scan, &structure);
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".nx")));
    }

    fn create_scan_with_files(files: Vec<&str>) -> ScanResult {
        ScanResult {
            repo_path: PathBuf::from("."),
            summary: RepoSummary {
                manifest_count: 1,
                primary_language: Some("Rust".to_string()),
                primary_build_system: Some("cargo".to_string()),
                is_monorepo: false,
                root_manifests: vec![],
            },
            detections: vec![DetectionStack::new(
                BuildSystemId::Cargo,
                LanguageId::Rust,
                PathBuf::from("Cargo.toml"),
            )
            .with_depth(0)
            .with_confidence(1.0)
            .with_workspace_root(false)],
            workspace: WorkspaceInfo {
                root_manifests: vec![],
                nested_by_depth: HashMap::new(),
                max_depth: 0,
                has_workspace_config: false,
            },
            file_tree: files.iter().map(PathBuf::from).collect(),
            scan_time_ms: 50,
        }
    }
}
