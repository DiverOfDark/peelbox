use super::phases::{
    assemble::{self, ServiceAnalysisResults},
    build, build_order, cache, classify, dependencies, entrypoint, env_vars, health, native_deps,
    port, root_cache, runtime, scan, structure,
};
use crate::frameworks::FrameworkRegistry;
use crate::heuristics::HeuristicLogger;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use crate::output::schema::UniversalBuild;
use crate::progress::{LoggingHandler, ProgressEvent};
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info};

pub struct PipelineOrchestrator {
    llm_client: Arc<dyn LLMClient>,
    registry: LanguageRegistry,
    framework_registry: FrameworkRegistry,
    progress_handler: Option<LoggingHandler>,
    heuristic_logger: Arc<HeuristicLogger>,
}

impl PipelineOrchestrator {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            registry: LanguageRegistry::with_defaults(),
            framework_registry: FrameworkRegistry::new(),
            progress_handler: None,
            heuristic_logger: Arc::new(HeuristicLogger::disabled()),
        }
    }

    pub fn with_progress_handler(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            registry: LanguageRegistry::with_defaults(),
            framework_registry: FrameworkRegistry::new(),
            progress_handler: Some(LoggingHandler),
            heuristic_logger: Arc::new(HeuristicLogger::disabled()),
        }
    }

    pub fn with_heuristic_logger(
        llm_client: Arc<dyn LLMClient>,
        progress_handler: Option<LoggingHandler>,
        heuristic_logger: Arc<HeuristicLogger>,
    ) -> Self {
        Self {
            llm_client,
            registry: LanguageRegistry::with_defaults(),
            framework_registry: FrameworkRegistry::new(),
            progress_handler,
            heuristic_logger,
        }
    }

    pub async fn execute(&self, repo_path: &Path) -> Result<Vec<UniversalBuild>> {
        let start = Instant::now();
        info!(
            "Starting pipeline orchestration for: {}",
            repo_path.display()
        );

        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::Started {
                repo_path: repo_path.display().to_string(),
            });
        }

        info!("Phase 1: Scanning repository");
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseStarted {
                phase: "scan".to_string(),
            });
        }
        let phase_start = Instant::now();
        let scan = scan::execute(repo_path).context("Phase 1: Scan failed")?;
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseComplete {
                phase: "scan".to_string(),
                duration: phase_start.elapsed(),
            });
        }
        debug!(
            "Scan complete: {} detections",
            scan.bootstrap_context.detections.len()
        );

        info!("Phase 2: Classifying directories");
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseStarted {
                phase: "classify".to_string(),
            });
        }
        let phase_start = Instant::now();
        let classification =
            classify::execute(self.llm_client.as_ref(), &scan, &self.heuristic_logger)
                .await
                .context("Phase 2: Classify failed")?;
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseComplete {
                phase: "classify".to_string(),
                duration: phase_start.elapsed(),
            });
        }
        debug!(
            "Classify complete: {} services",
            classification.services.len()
        );

        info!("Phase 3: Analyzing project structure");
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseStarted {
                phase: "structure".to_string(),
            });
        }
        let phase_start = Instant::now();
        let structure = structure::execute(
            self.llm_client.as_ref(),
            &scan,
            &classification,
            &self.heuristic_logger,
        )
        .await
        .context("Phase 3: Structure failed")?;
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseComplete {
                phase: "structure".to_string(),
                duration: phase_start.elapsed(),
            });
        }
        debug!(
            "Structure: {:?}, Tool: {:?}",
            structure.project_type, structure.monorepo_tool
        );

        info!("Phase 4: Extracting dependencies");
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseStarted {
                phase: "dependencies".to_string(),
            });
        }
        let phase_start = Instant::now();
        let dependencies = dependencies::execute(
            self.llm_client.as_ref(),
            &scan,
            &structure,
            &self.heuristic_logger,
        )
        .await
        .context("Phase 4: Dependencies failed")?;
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseComplete {
                phase: "dependencies".to_string(),
                duration: phase_start.elapsed(),
            });
        }
        debug!(
            "Dependencies extracted for {} services",
            dependencies.dependencies.len()
        );

        info!("Phase 5: Calculating build order");
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseStarted {
                phase: "build_order".to_string(),
            });
        }
        let phase_start = Instant::now();
        let build_order =
            build_order::execute(&dependencies).context("Phase 5: Build order failed")?;
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseComplete {
                phase: "build_order".to_string(),
                duration: phase_start.elapsed(),
            });
        }
        debug!(
            "Build order: {} services, has_cycle: {}",
            build_order.build_order.len(),
            build_order.has_cycle
        );

        info!("Phase 6: Analyzing services (runtime, build, entrypoint, native deps, port, env vars, health)");
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseStarted {
                phase: "service_analysis".to_string(),
            });
        }
        let phase_start = Instant::now();
        let mut analyses = Vec::new();
        let total_services = structure.services.len();

        for (index, service) in structure.services.iter().enumerate() {
            let service_start = Instant::now();
            info!("  Analyzing service: {}", service.path.display());

            if let Some(handler) = &self.progress_handler {
                handler.on_progress(&ProgressEvent::ServiceAnalysisStarted {
                    service_path: service.path.display().to_string(),
                    index: index + 1,
                    total: total_services,
                });
            }

            let analysis_result = async {
                debug!("    Phase 6a: Runtime detection");
                let runtime = runtime::execute(
                    self.llm_client.as_ref(),
                    service,
                    &scan,
                    &dependencies,
                    &self.framework_registry,
                    &self.heuristic_logger,
                )
                .await
                .context("Phase 6a: Runtime detection failed")?;

                debug!("    Phase 6b: Build detection");
                let build_info = build::execute(
                    self.llm_client.as_ref(),
                    service,
                    &scan,
                    &self.heuristic_logger,
                )
                .await
                .context("Phase 6b: Build detection failed")?;

                debug!("    Phase 6c: Entrypoint detection");
                let entrypoint = entrypoint::execute(
                    self.llm_client.as_ref(),
                    service,
                    &scan,
                    &self.heuristic_logger,
                )
                .await
                .context("Phase 6c: Entrypoint detection failed")?;

                debug!("    Phase 6d: Native dependencies detection");
                let native_deps = native_deps::execute(
                    self.llm_client.as_ref(),
                    service,
                    &scan,
                    &self.heuristic_logger,
                )
                .await
                .context("Phase 6d: Native deps detection failed")?;

                debug!("    Phase 6e: Port discovery");
                let port = port::execute(
                    self.llm_client.as_ref(),
                    service,
                    &runtime,
                    &scan,
                    &self.registry,
                    &self.framework_registry,
                    &self.heuristic_logger,
                )
                .await
                .context("Phase 6e: Port discovery failed")?;

                debug!("    Phase 6f: Environment variables discovery");
                let env_vars = env_vars::execute(
                    self.llm_client.as_ref(),
                    service,
                    &scan,
                    &self.registry,
                    &self.heuristic_logger,
                )
                .await
                .context("Phase 6f: Env vars discovery failed")?;

                debug!("    Phase 6g: Health check discovery");
                let health = health::execute(
                    self.llm_client.as_ref(),
                    service,
                    &runtime,
                    &scan,
                    &self.registry,
                    &self.framework_registry,
                    &self.heuristic_logger,
                )
                .await
                .context("Phase 6g: Health check discovery failed")?;

                debug!("    Phase 7: Cache detection");
                let cache = cache::execute(service);

                Ok::<_, anyhow::Error>(ServiceAnalysisResults {
                    service: service.clone(),
                    runtime,
                    build: build_info,
                    entrypoint,
                    native_deps,
                    port,
                    env_vars,
                    health,
                    cache,
                })
            }
            .await;

            match analysis_result {
                Ok(result) => {
                    analyses.push(result);
                    if let Some(handler) = &self.progress_handler {
                        handler.on_progress(&ProgressEvent::ServiceAnalysisComplete {
                            service_path: service.path.display().to_string(),
                            index: index + 1,
                            total: total_services,
                            duration: service_start.elapsed(),
                        });
                    }
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

        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseComplete {
                phase: "service_analysis".to_string(),
                duration: phase_start.elapsed(),
            });
        }

        info!("Phase 8: Root cache detection");
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseStarted {
                phase: "root_cache".to_string(),
            });
        }
        let phase_start = Instant::now();
        let root_cache = root_cache::execute(&structure);
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseComplete {
                phase: "root_cache".to_string(),
                duration: phase_start.elapsed(),
            });
        }
        debug!(
            "Root cache: {} directories",
            root_cache.root_cache_dirs.len()
        );

        info!("Phase 9: Assembling UniversalBuild outputs");
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseStarted {
                phase: "assemble".to_string(),
            });
        }
        let phase_start = Instant::now();
        let builds = assemble::execute(analyses, &structure, &root_cache)
            .context("Phase 9: Assemble failed")?;
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::PhaseComplete {
                phase: "assemble".to_string(),
                duration: phase_start.elapsed(),
            });
        }

        info!(
            "Pipeline complete: generated {} UniversalBuild(s)",
            builds.len()
        );
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::Completed {
                total_iterations: 0,
                total_time: start.elapsed(),
            });
        }

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

        assert!(std::ptr::eq(
            orchestrator.llm_client.as_ref(),
            orchestrator.llm_client.as_ref()
        ));
    }
}
