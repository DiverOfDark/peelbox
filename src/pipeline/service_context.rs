use super::context::AnalysisContext;
use super::phases::{
    dependencies::DependencyResult, runtime::RuntimeInfo, scan::ScanResult, structure::Service,
};
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use crate::stack::StackRegistry;
use std::path::Path;
use std::sync::Arc;

pub struct ServiceContext<'a> {
    pub service: &'a Service,
    pub analysis_context: &'a AnalysisContext,
    pub runtime: Option<&'a RuntimeInfo>,
}

impl<'a> ServiceContext<'a> {
    pub fn new(service: &'a Service, analysis_context: &'a AnalysisContext) -> Self {
        Self {
            service,
            analysis_context,
            runtime: None,
        }
    }

    pub fn with_runtime(&mut self, runtime: &'a RuntimeInfo) {
        self.runtime = Some(runtime);
    }

    pub fn repo_path(&self) -> &Path {
        &self.analysis_context.repo_path
    }

    pub fn scan(&self) -> &ScanResult {
        self.analysis_context
            .scan
            .as_ref()
            .expect("Scan result must be available before service analysis")
    }

    pub fn dependencies(&self) -> &DependencyResult {
        self.analysis_context
            .dependencies
            .as_ref()
            .expect("Dependencies must be available before service analysis")
    }

    pub fn llm_client(&self) -> &dyn LLMClient {
        self.analysis_context.llm_client.as_ref()
    }

    pub fn stack_registry(&self) -> &Arc<StackRegistry> {
        &self.analysis_context.stack_registry
    }

    pub fn heuristic_logger(&self) -> &Arc<HeuristicLogger> {
        &self.analysis_context.heuristic_logger
    }
}
