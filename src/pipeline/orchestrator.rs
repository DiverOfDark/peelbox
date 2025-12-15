use super::phases::{
    assemble::{self, ServiceAnalysisResults},
    build, build_order, cache, classify, dependencies, entrypoint, env_vars, health, native_deps,
    port, root_cache, runtime, scan, structure,
};
use crate::llm::LLMClient;
use crate::output::schema::UniversalBuild;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

pub struct PipelineOrchestrator {
    llm_client: Arc<dyn LLMClient>,
}

impl PipelineOrchestrator {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self { llm_client }
    }

    pub async fn execute(&self, repo_path: &Path) -> Result<Vec<UniversalBuild>> {
        info!("Starting pipeline orchestration for: {}", repo_path.display());

        info!("Phase 1: Scanning repository");
        let scan_result = scan::execute(repo_path).context("Phase 1: Scan failed")?;
        debug!("Scan complete: {} detections", scan_result.bootstrap_context.detections.len());

        info!("Phase 2: Classifying directories");
        let classify_result = classify::execute(self.llm_client.as_ref(), &scan_result)
            .await
            .context("Phase 2: Classify failed")?;
        debug!("Classify complete: {} services", classify_result.services.len());

        info!("Phase 3: Analyzing project structure");
        let structure_result = structure::execute(self.llm_client.as_ref(), &scan_result, &classify_result)
            .await
            .context("Phase 3: Structure failed")?;
        debug!("Structure: {:?}, Tool: {:?}", structure_result.project_type, structure_result.monorepo_tool);

        info!("Phase 4: Extracting dependencies");
        let dependency_result = dependencies::execute(
            self.llm_client.as_ref(),
            &scan_result,
            &structure_result,
        )
        .await
        .context("Phase 4: Dependencies failed")?;
        debug!("Dependencies extracted for {} services", dependency_result.dependencies.len());

        info!("Phase 5: Calculating build order");
        let build_order_result = build_order::execute(&dependency_result)
            .context("Phase 5: Build order failed")?;
        debug!("Build order: {} services, has_cycle: {}", build_order_result.build_order.len(), build_order_result.has_cycle);

        info!("Phase 6: Analyzing services (runtime, build, entrypoint, native deps, port, env vars, health)");
        let mut service_analysis_results = Vec::new();

        for service in &structure_result.services {
            info!("  Analyzing service: {}", service.path.display());

            debug!("    Phase 6a: Runtime detection");
            let runtime_info = runtime::execute(self.llm_client.as_ref(), service, &scan_result)
                .await
                .context("Phase 6a: Runtime detection failed")?;

            debug!("    Phase 6b: Build detection");
            let build_info = build::execute(self.llm_client.as_ref(), service, &scan_result)
                .await
                .context("Phase 6b: Build detection failed")?;

            debug!("    Phase 6c: Entrypoint detection");
            let entrypoint_info = entrypoint::execute(self.llm_client.as_ref(), service, &scan_result)
                .await
                .context("Phase 6c: Entrypoint detection failed")?;

            debug!("    Phase 6d: Native dependencies detection");
            let native_deps_info = native_deps::execute(self.llm_client.as_ref(), service, &scan_result)
                .await
                .context("Phase 6d: Native deps detection failed")?;

            debug!("    Phase 6e: Port discovery");
            let port_info = port::execute(self.llm_client.as_ref(), service, &scan_result)
                .await
                .context("Phase 6e: Port discovery failed")?;

            debug!("    Phase 6f: Environment variables discovery");
            let env_vars_info = env_vars::execute(self.llm_client.as_ref(), service, &scan_result)
                .await
                .context("Phase 6f: Env vars discovery failed")?;

            debug!("    Phase 6g: Health check discovery");
            let health_info = health::execute(self.llm_client.as_ref(), service, &runtime_info, &scan_result)
                .await
                .context("Phase 6g: Health check discovery failed")?;

            debug!("    Phase 7: Cache detection");
            let cache_info = cache::execute(service);

            service_analysis_results.push(ServiceAnalysisResults {
                service: service.clone(),
                runtime: runtime_info,
                build: build_info,
                entrypoint: entrypoint_info,
                native_deps: native_deps_info,
                port: port_info,
                env_vars: env_vars_info,
                health: health_info,
                cache: cache_info,
            });
        }

        info!("Phase 8: Root cache detection");
        let root_cache_info = root_cache::execute(&structure_result);
        debug!("Root cache: {} directories", root_cache_info.root_cache_dirs.len());

        info!("Phase 9: Assembling UniversalBuild outputs");
        let builds = assemble::execute(
            service_analysis_results,
            &structure_result,
            &root_cache_info,
        )
        .context("Phase 9: Assemble failed")?;

        info!("Pipeline complete: generated {} UniversalBuild(s)", builds.len());
        Ok(builds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MockLLMClient;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let mock_client = Arc::new(MockLLMClient::new());
        let orchestrator = PipelineOrchestrator::new(mock_client);

        assert!(std::ptr::eq(orchestrator.llm_client.as_ref(), orchestrator.llm_client.as_ref()));
    }
}
