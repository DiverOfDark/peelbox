use super::context::AnalysisContext;
use super::service_context::ServiceContext;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait WorkflowPhase: Send + Sync {
    async fn execute(&self, context: &mut AnalysisContext) -> Result<()>;
}

pub enum ServicePhaseResult {
    Runtime(super::phases::runtime::RuntimeInfo),
    Build(super::phases::build::BuildInfo),
    Entrypoint(super::phases::entrypoint::EntrypointInfo),
    NativeDeps(super::phases::native_deps::NativeDepsInfo),
    Port(super::phases::port::PortInfo),
    EnvVars(super::phases::env_vars::EnvVarsInfo),
    Health(super::phases::health::HealthInfo),
    Cache(super::phases::cache::CacheInfo),
}

#[async_trait]
pub trait ServicePhase: Send + Sync {
    async fn execute(&self, context: &ServiceContext<'_>) -> Result<ServicePhaseResult>;
}
