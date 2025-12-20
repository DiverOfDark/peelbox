use super::phases::{
    build_order::BuildOrderResult, dependencies::DependencyResult, root_cache::RootCacheInfo,
    scan::ScanResult,
};
use super::service_context::ServiceContext;
use crate::config::DetectionMode;
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use crate::output::schema::UniversalBuild;
use crate::progress::LoggingHandler;
use crate::stack::orchestrator::WorkspaceStructure;
use crate::stack::StackRegistry;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct AnalysisContext {
    pub repo_path: PathBuf,
    pub llm_client: Arc<dyn LLMClient>,
    pub stack_registry: Arc<StackRegistry>,
    pub progress_handler: Option<LoggingHandler>,
    pub heuristic_logger: Arc<HeuristicLogger>,
    pub detection_mode: DetectionMode,
    pub scan: Option<ScanResult>,
    pub workspace: Option<WorkspaceStructure>,
    pub dependencies: Option<DependencyResult>,
    pub build_order: Option<BuildOrderResult>,
    pub root_cache: Option<RootCacheInfo>,
    pub service_analyses: Vec<ServiceContext>,
    pub builds: Vec<UniversalBuild>,
}

impl AnalysisContext {
    pub fn new(
        repo_path: &Path,
        llm_client: Arc<dyn LLMClient>,
        stack_registry: Arc<StackRegistry>,
        progress_handler: Option<LoggingHandler>,
        heuristic_logger: Arc<HeuristicLogger>,
        detection_mode: DetectionMode,
    ) -> Self {
        Self {
            repo_path: repo_path.to_path_buf(),
            llm_client,
            stack_registry,
            progress_handler,
            heuristic_logger,
            detection_mode,
            scan: None,
            workspace: None,
            dependencies: None,
            build_order: None,
            root_cache: None,
            service_analyses: Vec::new(),
            builds: Vec::new(),
        }
    }
}
