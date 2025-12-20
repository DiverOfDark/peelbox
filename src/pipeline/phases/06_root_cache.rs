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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::context::AnalysisContext;
    use crate::pipeline::phase_trait::WorkflowPhase;
    use crate::pipeline::phases::scan::{RepoSummary, ScanResult, WorkspaceInfo};
    use crate::pipeline::Confidence;
    use crate::stack::orchestrator::{OrchestratorId, WorkspaceStructure};
    use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_root_cache_pnpm() {
        let mut scan = create_scan_with_files(vec!["pnpm-workspace.yaml"]);
        scan.detections[0].build_system = BuildSystemId::Pnpm;
        scan.detections[0].is_workspace_root = true;

        let workspace = WorkspaceStructure {
            orchestrator: Some(OrchestratorId::Turborepo),
            packages: vec![],
        };

        let result = execute_phase(&scan, &workspace).await;
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from(".pnpm-store")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[tokio::test]
    async fn test_root_cache_cargo_workspace() {
        let mut scan = create_scan_with_files(vec!["Cargo.toml"]);
        scan.detections[0].is_workspace_root = true;

        let workspace = WorkspaceStructure {
            orchestrator: Some(OrchestratorId::Turborepo),
            packages: vec![],
        };

        let result = execute_phase(&scan, &workspace).await;
        assert!(result.root_cache_dirs.contains(&PathBuf::from("target")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".cargo")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[tokio::test]
    async fn test_root_cache_turborepo() {
        let scan = create_scan_with_files(vec!["turbo.json", "package.json"]);

        // Create a monorepo with 2+ packages to trigger orchestrator cache detection
        let workspace = WorkspaceStructure {
            orchestrator: Some(OrchestratorId::Turborepo),
            packages: vec![
                crate::stack::orchestrator::Package {
                    path: PathBuf::from("apps/web"),
                    name: "web".to_string(),
                    is_application: true,
                },
                crate::stack::orchestrator::Package {
                    path: PathBuf::from("apps/api"),
                    name: "api".to_string(),
                    is_application: true,
                },
            ],
        };

        let result = execute_phase(&scan, &workspace).await;
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".turbo")));
    }

    #[tokio::test]
    async fn test_root_cache_none() {
        let scan = create_scan_with_files(vec!["package.json"]);

        let workspace = WorkspaceStructure {
            orchestrator: None,
            packages: vec![],
        };

        let result = execute_phase(&scan, &workspace).await;
        assert!(result.root_cache_dirs.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }

    #[tokio::test]
    async fn test_root_cache_nx() {
        let scan = create_scan_with_files(vec!["nx.json", "package.json"]);

        // Create a monorepo with 2+ packages to trigger orchestrator cache detection
        let workspace = WorkspaceStructure {
            orchestrator: Some(OrchestratorId::Nx),
            packages: vec![
                crate::stack::orchestrator::Package {
                    path: PathBuf::from("apps/web"),
                    name: "web".to_string(),
                    is_application: true,
                },
                crate::stack::orchestrator::Package {
                    path: PathBuf::from("libs/shared"),
                    name: "shared".to_string(),
                    is_application: false,
                },
            ],
        };

        let result = execute_phase(&scan, &workspace).await;
        assert!(result
            .root_cache_dirs
            .contains(&PathBuf::from("node_modules")));
        assert!(result.root_cache_dirs.contains(&PathBuf::from(".nx")));
    }

    async fn execute_phase(scan: &ScanResult, workspace: &WorkspaceStructure) -> RootCacheInfo {
        use crate::heuristics::HeuristicLogger;
        use crate::llm::MockLLMClient;
        use crate::stack::StackRegistry;
        use std::sync::Arc;

        use crate::config::DetectionMode;
        let llm_client: Arc<dyn crate::llm::LLMClient> = Arc::new(MockLLMClient::default());
        let stack_registry = Arc::new(StackRegistry::with_defaults());
        let heuristic_logger = Arc::new(HeuristicLogger::new(None));

        let mut context = AnalysisContext::new(
            &PathBuf::from("."),
            llm_client,
            stack_registry,
            None,
            heuristic_logger,
            DetectionMode::Full,
        );
        context.scan = Some(scan.clone());
        context.workspace = Some(workspace.clone());

        let phase = RootCachePhase;
        phase.try_deterministic(&mut context).unwrap().unwrap();

        context.root_cache.unwrap()
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

use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use anyhow::Result;
use async_trait::async_trait;

pub struct RootCachePhase;

#[async_trait]
impl WorkflowPhase for RootCachePhase {
    fn name(&self) -> &'static str {
        "RootCachePhase"
    }

    fn try_deterministic(&self, context: &mut AnalysisContext) -> Result<Option<()>> {
        self.execute_root_cache(context)?;
        Ok(Some(()))
    }

    async fn execute_llm(&self, context: &mut AnalysisContext) -> Result<()> {
        self.execute_root_cache(context)
    }
}

impl RootCachePhase {
    fn execute_root_cache(&self, context: &mut AnalysisContext) -> Result<()> {
        let scan = context
            .scan
            .as_ref()
            .expect("Scan must be available before root_cache");
        let workspace = context
            .workspace
            .as_ref()
            .expect("Workspace must be available before root_cache");

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

        // Add cache dirs from orchestrator (only for actual monorepos with > 1 package and an orchestrator)
        if workspace.packages.len() > 1 {
            if let Some(orchestrator_id) = workspace.orchestrator {
                for orchestrator in registry.all_orchestrators() {
                    if orchestrator.id() == orchestrator_id {
                        for cache_dir in orchestrator.cache_dirs() {
                            root_cache_dirs.insert(PathBuf::from(cache_dir));
                        }
                        break;
                    }
                }
            }
        }

        let mut dirs: Vec<PathBuf> = root_cache_dirs.into_iter().collect();
        dirs.sort();

        let result = RootCacheInfo {
            root_cache_dirs: dirs,
            confidence: Confidence::High,
        };

        context.root_cache = Some(result);
        Ok(())
    }
}
