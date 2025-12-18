use super::phases::{
    assemble::ServiceAnalysisResults, build_order::BuildOrderResult, classify::ClassifyResult,
    dependencies::DependencyResult, root_cache::RootCacheInfo, scan::ScanResult,
    structure::StructureResult,
};
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use crate::output::schema::UniversalBuild;
use crate::progress::LoggingHandler;
use crate::stack::StackRegistry;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct AnalysisContext {
    pub repo_path: PathBuf,
    pub llm_client: Arc<dyn LLMClient>,
    pub stack_registry: Arc<StackRegistry>,
    pub progress_handler: Option<LoggingHandler>,
    pub heuristic_logger: Arc<HeuristicLogger>,
    pub scan: Option<ScanResult>,
    pub classify: Option<ClassifyResult>,
    pub structure: Option<StructureResult>,
    pub dependencies: Option<DependencyResult>,
    pub build_order: Option<BuildOrderResult>,
    pub root_cache: Option<RootCacheInfo>,
    pub service_analyses: Vec<ServiceAnalysisResults>,
    pub builds: Vec<UniversalBuild>,
}

impl AnalysisContext {
    pub fn new(
        repo_path: &Path,
        llm_client: Arc<dyn LLMClient>,
        stack_registry: Arc<StackRegistry>,
        progress_handler: Option<LoggingHandler>,
        heuristic_logger: Arc<HeuristicLogger>,
    ) -> Self {
        Self {
            repo_path: repo_path.to_path_buf(),
            llm_client,
            stack_registry,
            progress_handler,
            heuristic_logger,
            scan: None,
            classify: None,
            structure: None,
            dependencies: None,
            build_order: None,
            root_cache: None,
            service_analyses: Vec::new(),
            builds: Vec::new(),
        }
    }
}
