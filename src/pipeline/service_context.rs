use super::context::AnalysisContext;
use super::phases::{
    build::BuildInfo, cache::CacheInfo, dependencies::DependencyResult, entrypoint::EntrypointInfo,
    env_vars::EnvVarsInfo, health::HealthInfo, native_deps::NativeDepsInfo, port::PortInfo,
    runtime::RuntimeInfo, scan::ScanResult, structure::Service,
};
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use crate::stack::StackRegistry;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

pub struct ServiceContext<'a> {
    pub service: &'a Service,
    pub analysis_context: &'a AnalysisContext,

    // Phase results
    pub runtime: Option<RuntimeInfo>,
    pub build: Option<BuildInfo>,
    pub entrypoint: Option<EntrypointInfo>,
    pub native_deps: Option<NativeDepsInfo>,
    pub port: Option<PortInfo>,
    pub env_vars: Option<EnvVarsInfo>,
    pub health: Option<HealthInfo>,
    pub cache: Option<CacheInfo>,
}

/// Completed service analysis with all phase results
pub struct OwnedServiceContext {
    pub service: Service,
    pub runtime: RuntimeInfo,
    pub build: BuildInfo,
    pub entrypoint: EntrypointInfo,
    pub native_deps: NativeDepsInfo,
    pub port: PortInfo,
    pub env_vars: EnvVarsInfo,
    pub health: HealthInfo,
    pub cache: CacheInfo,
}

impl<'a> ServiceContext<'a> {
    pub fn new(service: &'a Service, analysis_context: &'a AnalysisContext) -> Self {
        Self {
            service,
            analysis_context,
            runtime: None,
            build: None,
            entrypoint: None,
            native_deps: None,
            port: None,
            env_vars: None,
            health: None,
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

    pub fn dependencies(&self) -> Result<&DependencyResult> {
        self.analysis_context
            .dependencies
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Dependencies must be available before service analysis"))
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

    /// Convert to completed service analysis (consumes self and clones service)
    pub fn into_owned(self) -> OwnedServiceContext {
        OwnedServiceContext {
            service: self.service.clone(),
            runtime: self.runtime.expect("Runtime must be set after RuntimePhase"),
            build: self.build.expect("Build must be set after BuildPhase"),
            entrypoint: self
                .entrypoint
                .expect("Entrypoint must be set after EntrypointPhase"),
            native_deps: self
                .native_deps
                .expect("NativeDeps must be set after NativeDepsPhase"),
            port: self.port.expect("Port must be set after PortPhase"),
            env_vars: self
                .env_vars
                .expect("EnvVars must be set after EnvVarsPhase"),
            health: self.health.expect("Health must be set after HealthPhase"),
            cache: self.cache.expect("Cache must be set after CachePhase"),
        }
    }
}
