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
use std::sync::Arc;

pub struct ServiceAnalysisPhase;

#[async_trait]
impl WorkflowPhase for ServiceAnalysisPhase {
    fn name(&self) -> &'static str {
        "ServiceAnalysisPhase"
    }

    async fn execute_llm(&self, context: &mut AnalysisContext) -> Result<()> {
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
    ) -> Result<ServiceContext> {
        let service_arc = Arc::new(service.clone());
        let context_arc = Arc::new((*context).clone());
        let mut service_context = ServiceContext::new(service_arc, context_arc);

        // Execute all service phases in order
        let phases: Vec<&dyn ServicePhase> = vec![
            &RuntimePhase,
            &BuildPhase,
            &EntrypointPhase,
            &NativeDepsPhase,
            &PortPhase,
            &EnvVarsPhase,
            &HealthPhase,
            &CachePhase,
        ];

        for phase in phases {
            phase
                .execute(&mut service_context)
                .await
                .with_context(|| {
                    format!(
                        "{} failed for service at {}",
                        phase.name(),
                        service.path.display()
                    )
                })?;
        }

        Ok(service_context)
    }
}
