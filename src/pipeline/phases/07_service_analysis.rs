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
use crate::pipeline::phase_trait::{ServicePhase, ServicePhaseResult, WorkflowPhase};
use crate::pipeline::service_context::ServiceContext;
use anyhow::{Context as AnyhowContext, Result};
use async_trait::async_trait;

pub struct ServiceAnalysisPhase;

#[async_trait]
impl WorkflowPhase for ServiceAnalysisPhase {
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
        let runtime_result = runtime_phase
            .execute(&service_context)
            .await
            .context("Runtime detection failed")?;
        let runtime = match runtime_result {
            ServicePhaseResult::Runtime(r) => r,
            _ => unreachable!(),
        };

        service_context.with_runtime(&runtime);

        let build_phase = BuildPhase;
        let build_result = build_phase
            .execute(&service_context)
            .await
            .context("Build detection failed")?;
        let build = match build_result {
            ServicePhaseResult::Build(b) => b,
            _ => unreachable!(),
        };

        let entrypoint_phase = EntrypointPhase;
        let entrypoint_result = entrypoint_phase
            .execute(&service_context)
            .await
            .context("Entrypoint detection failed")?;
        let entrypoint = match entrypoint_result {
            ServicePhaseResult::Entrypoint(e) => e,
            _ => unreachable!(),
        };

        let native_deps_phase = NativeDepsPhase;
        let native_deps_result = native_deps_phase
            .execute(&service_context)
            .await
            .context("Native deps detection failed")?;
        let native_deps = match native_deps_result {
            ServicePhaseResult::NativeDeps(n) => n,
            _ => unreachable!(),
        };

        let port_phase = PortPhase;
        let port_result = port_phase
            .execute(&service_context)
            .await
            .context("Port discovery failed")?;
        let port = match port_result {
            ServicePhaseResult::Port(p) => p,
            _ => unreachable!(),
        };

        let env_vars_phase = EnvVarsPhase;
        let env_vars_result = env_vars_phase
            .execute(&service_context)
            .await
            .context("Env vars discovery failed")?;
        let env_vars = match env_vars_result {
            ServicePhaseResult::EnvVars(e) => e,
            _ => unreachable!(),
        };

        let health_phase = HealthPhase;
        let health_result = health_phase
            .execute(&service_context)
            .await
            .context("Health check discovery failed")?;
        let health = match health_result {
            ServicePhaseResult::Health(h) => h,
            _ => unreachable!(),
        };

        let cache_phase = CachePhase;
        let cache_result = cache_phase
            .execute(&service_context)
            .await
            .context("Cache detection failed")?;
        let cache = match cache_result {
            ServicePhaseResult::Cache(c) => c,
            _ => unreachable!(),
        };

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
