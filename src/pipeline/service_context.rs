use super::context::AnalysisContext;
use super::phases::{
    build::BuildInfo, cache::CacheInfo, scan::ScanResult, service_analysis::Service,
};
use crate::heuristics::HeuristicLogger;
use crate::stack::runtime::RuntimeConfig;
use crate::stack::{BuildSystemId, FrameworkId, LanguageId, RuntimeId, StackRegistry};
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

/// Complete technology stack for a service
#[derive(Clone, Debug)]
pub struct Stack {
    pub language: LanguageId,
    pub build_system: BuildSystemId,
    pub framework: Option<FrameworkId>,
    pub runtime: RuntimeId,
    pub version: Option<String>,
}

#[derive(Clone)]
pub struct ServiceContext {
    pub service: Arc<Service>,
    pub analysis_context: Arc<AnalysisContext>,

    // Phase results
    pub stack: Option<Stack>,
    pub runtime_config: Option<RuntimeConfig>,
    pub build: Option<BuildInfo>,
    pub cache: Option<CacheInfo>,
}

impl ServiceContext {
    pub fn new(service: Arc<Service>, analysis_context: Arc<AnalysisContext>) -> Self {
        Self {
            service,
            analysis_context,
            stack: None,
            runtime_config: None,
            build: None,
            cache: None,
        }
    }

    pub fn repo_path(&self) -> &Path {
        &self.analysis_context.repo_path
    }

    pub fn scan(&self) -> Result<&ScanResult> {
        self.analysis_context
            .scan
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Scan result must be available before service analysis"))
    }

    pub fn stack_registry(&self) -> &Arc<StackRegistry> {
        &self.analysis_context.stack_registry
    }

    pub fn heuristic_logger(&self) -> &Arc<HeuristicLogger> {
        &self.analysis_context.heuristic_logger
    }
}
