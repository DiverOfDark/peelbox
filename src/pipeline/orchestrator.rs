use super::phases::{
    assemble::{self, ServiceAnalysisResults},
    build, build_order, cache, classify, dependencies, entrypoint, env_vars, health, native_deps,
    port, root_cache, runtime, scan, structure,
};
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use crate::output::schema::UniversalBuild;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

pub struct PipelineOrchestrator {
    llm_client: Arc<dyn LLMClient>,
    registry: LanguageRegistry,
}

impl PipelineOrchestrator {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            registry: LanguageRegistry::with_defaults(),
        }
    }

    pub async fn execute(&self, repo_path: &Path) -> Result<Vec<UniversalBuild>> {
        info!("Starting pipeline orchestration for: {}", repo_path.display());

        info!("Phase 1: Scanning repository");
        let scan = scan::execute(repo_path).context("Phase 1: Scan failed")?;
        debug!("Scan complete: {} detections", scan.bootstrap_context.detections.len());

        info!("Phase 2: Classifying directories");
        let classification = classify::execute(self.llm_client.as_ref(), &scan)
            .await
            .context("Phase 2: Classify failed")?;
        debug!("Classify complete: {} services", classification.services.len());

        info!("Phase 3: Analyzing project structure");
        let structure = structure::execute(self.llm_client.as_ref(), &scan, &classification)
            .await
            .context("Phase 3: Structure failed")?;
        debug!("Structure: {:?}, Tool: {:?}", structure.project_type, structure.monorepo_tool);

        info!("Phase 4: Extracting dependencies");
        let dependencies = dependencies::execute(
            self.llm_client.as_ref(),
            &scan,
            &structure,
        )
        .await
        .context("Phase 4: Dependencies failed")?;
        debug!("Dependencies extracted for {} services", dependencies.dependencies.len());

        info!("Phase 5: Calculating build order");
        let build_order = build_order::execute(&dependencies)
            .context("Phase 5: Build order failed")?;
        debug!("Build order: {} services, has_cycle: {}", build_order.build_order.len(), build_order.has_cycle);

        info!("Phase 6: Analyzing services (runtime, build, entrypoint, native deps, port, env vars, health)");
        let mut analyses = Vec::new();

        for service in &structure.services {
            info!("  Analyzing service: {}", service.path.display());

            debug!("    Phase 6a: Runtime detection");
            let runtime = runtime::execute(self.llm_client.as_ref(), service, &scan)
                .await
                .context("Phase 6a: Runtime detection failed")?;

            debug!("    Phase 6b: Build detection");
            let build_info = build::execute(self.llm_client.as_ref(), service, &scan)
                .await
                .context("Phase 6b: Build detection failed")?;

            debug!("    Phase 6c: Entrypoint detection");
            let entrypoint = entrypoint::execute(self.llm_client.as_ref(), service, &scan)
                .await
                .context("Phase 6c: Entrypoint detection failed")?;

            debug!("    Phase 6d: Native dependencies detection");
            let native_deps = native_deps::execute(self.llm_client.as_ref(), service, &scan)
                .await
                .context("Phase 6d: Native deps detection failed")?;

            debug!("    Phase 6e: Port discovery");
            let port = port::execute(self.llm_client.as_ref(), service, &scan, &self.registry)
                .await
                .context("Phase 6e: Port discovery failed")?;

            debug!("    Phase 6f: Environment variables discovery");
            let env_vars = env_vars::execute(self.llm_client.as_ref(), service, &scan, &self.registry)
                .await
                .context("Phase 6f: Env vars discovery failed")?;

            debug!("    Phase 6g: Health check discovery");
            let health = health::execute(self.llm_client.as_ref(), service, &runtime, &scan, &self.registry)
                .await
                .context("Phase 6g: Health check discovery failed")?;

            debug!("    Phase 7: Cache detection");
            let cache = cache::execute(service);

            analyses.push(ServiceAnalysisResults {
                service: service.clone(),
                runtime,
                build: build_info,
                entrypoint,
                native_deps,
                port,
                env_vars,
                health,
                cache,
            });
        }

        info!("Phase 8: Root cache detection");
        let root_cache = root_cache::execute(&structure);
        debug!("Root cache: {} directories", root_cache.root_cache_dirs.len());

        info!("Phase 9: Assembling UniversalBuild outputs");
        let builds = assemble::execute(
            analyses,
            &structure,
            &root_cache,
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
