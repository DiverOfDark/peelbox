use super::assemble::ServiceAnalysisResults;
use super::build::BuildPhase;
use super::cache::CachePhase;
use super::entrypoint::EntrypointPhase;
use super::env_vars::EnvVarsPhase;
use super::health::HealthPhase;
use super::native_deps::NativeDepsPhase;
use super::port::PortPhase;
use super::runtime::RuntimePhase;
use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::{ServicePhase, WorkflowPhase};
use crate::pipeline::service_context::ServiceContext;
use anyhow::{Context as AnyhowContext, Result};
use async_trait::async_trait;

pub struct ServiceAnalysisPhase;

#[async_trait]
impl WorkflowPhase for ServiceAnalysisPhase {
    fn name(&self) -> &'static str {
        "ServiceAnalysisPhase"
    }

    async fn execute(&self, context: &mut AnalysisContext) -> Result<()> {
        let structure = context
            .structure
            .as_ref()
            .expect("Structure must be available before service analysis");

        for service in &structure.services {
            let analysis_result = self.analyze_service(service, context).await;

            match analysis_result {
                Ok(result) => {
                    context.service_analyses.push(result);
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to analyze service {}: {}. Skipping service.",
                        service.path.display(),
                        e
                    );
                }
            }
        }

        Ok(())
    }
}

impl ServiceAnalysisPhase {
    async fn analyze_service(
        &self,
        service: &super::structure::Service,
        context: &AnalysisContext,
    ) -> Result<ServiceAnalysisResults> {
        let mut service_context = ServiceContext::new(service, context);

        let runtime_phase = RuntimePhase;
        let runtime = runtime_phase
            .execute(&service_context)
            .await
            .context("Runtime detection failed")?;

        service_context.with_runtime(&runtime);

        let build_phase = BuildPhase;
        let build = build_phase
            .execute(&service_context)
            .await
            .context("Build detection failed")?;

        let entrypoint_phase = EntrypointPhase;
        let entrypoint = entrypoint_phase
            .execute(&service_context)
            .await
            .context("Entrypoint detection failed")?;

        let native_deps_phase = NativeDepsPhase;
        let native_deps = native_deps_phase
            .execute(&service_context)
            .await
            .context("Native deps detection failed")?;

        let port_phase = PortPhase;
        let port = port_phase
            .execute(&service_context)
            .await
            .context("Port discovery failed")?;

        let env_vars_phase = EnvVarsPhase;
        let env_vars = env_vars_phase
            .execute(&service_context)
            .await
            .context("Env vars discovery failed")?;

        let health_phase = HealthPhase;
        let health = health_phase
            .execute(&service_context)
            .await
            .context("Health check discovery failed")?;

        let cache_phase = CachePhase;
        let cache = cache_phase
            .execute(&service_context)
            .await
            .context("Cache detection failed")?;

        Ok(ServiceAnalysisResults {
            service: service.clone(),
            runtime,
            build,
            entrypoint,
            native_deps,
            port,
            env_vars,
            health,
            cache,
        })
    }
}
