use super::phases::{root_cache::RootCacheInfo, scan::ScanResult};
use super::service_context::ServiceContext;
use peelbox_core::config::DetectionMode;
use peelbox_core::output::schema::UniversalBuild;
use peelbox_stack::orchestrator::WorkspaceStructure;
use peelbox_stack::StackRegistry;
use peelbox_wolfi::WolfiPackageIndex;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct AnalysisContext {
    pub repo_path: PathBuf,
    pub stack_registry: Arc<StackRegistry>,
    pub wolfi_index: Arc<WolfiPackageIndex>,
    pub detection_mode: DetectionMode,
    pub scan: Option<ScanResult>,
    pub workspace: Option<WorkspaceStructure>,
    pub root_cache: Option<RootCacheInfo>,
    pub service_analyses: Vec<ServiceContext>,
    pub builds: Vec<UniversalBuild>,
}

impl AnalysisContext {
    pub fn new(
        repo_path: &Path,
        stack_registry: Arc<StackRegistry>,
        wolfi_index: Arc<WolfiPackageIndex>,
        detection_mode: DetectionMode,
    ) -> Self {
        Self {
            repo_path: repo_path.to_path_buf(),
            stack_registry,
            wolfi_index,
            detection_mode,
            scan: None,
            workspace: None,
            root_cache: None,
            service_analyses: Vec::new(),
            builds: Vec::new(),
        }
    }
}
