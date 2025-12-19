use super::context::AnalysisContext;
use super::phase_trait::WorkflowPhase;
use super::phases::{
    assemble::AssemblePhase,
    build_order::BuildOrderPhase,
    classify::ClassifyPhase,
    dependencies::DependenciesPhase,
    root_cache::RootCachePhase,
    scan::ScanPhase,
    service_analysis::ServiceAnalysisPhase,
    structure::StructurePhase,
};
use crate::output::schema::UniversalBuild;
use crate::progress::{LoggingHandler, ProgressEvent};
use anyhow::{Context, Result};
use std::path::Path;
use std::time::Instant;
use tracing::{debug, info};

pub struct PipelineOrchestrator {
    progress_handler: Option<LoggingHandler>,
}

impl PipelineOrchestrator {
    pub fn new(progress_handler: Option<LoggingHandler>) -> Self {
        Self { progress_handler }
    }

    async fn execute_phase(
        &self,
        phase: Box<dyn WorkflowPhase>,
        context: &mut AnalysisContext,
    ) -> Result<()> {
        phase.execute(context).await
    }

    pub async fn execute(
        &self,
        repo_path: &Path,
        context: &mut AnalysisContext,
    ) -> Result<Vec<UniversalBuild>> {
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

        let workflow_phases: Vec<Box<dyn WorkflowPhase>> = vec![
            Box::new(ScanPhase),
            Box::new(ClassifyPhase),
            Box::new(StructurePhase),
            Box::new(DependenciesPhase),
            Box::new(BuildOrderPhase),
            Box::new(RootCachePhase),
            Box::new(ServiceAnalysisPhase),
            Box::new(AssemblePhase),
        ];

        for phase in workflow_phases {
            let phase_name = phase.name();
            info!("Phase: {}", phase_name);

            if let Some(handler) = &self.progress_handler {
                handler.on_progress(&ProgressEvent::PhaseStarted {
                    phase: phase_name.to_string(),
                });
            }

            let phase_start = Instant::now();
            self.execute_phase(phase, context)
                .await
                .with_context(|| format!("Phase {} failed", phase_name))?;

            if let Some(handler) = &self.progress_handler {
                handler.on_progress(&ProgressEvent::PhaseComplete {
                    phase: phase_name.to_string(),
                    duration: phase_start.elapsed(),
                });
            }

            debug!("Phase {} complete", phase_name);
        }

        info!(
            "Pipeline complete: generated {} UniversalBuild(s)",
            context.builds.len()
        );
        if let Some(handler) = &self.progress_handler {
            handler.on_progress(&ProgressEvent::Completed {
                total_iterations: 0,
                total_time: start.elapsed(),
            });
        }

        Ok(std::mem::take(&mut context.builds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let orchestrator = PipelineOrchestrator::new(None);
        assert!(orchestrator.progress_handler.is_none());
    }

    #[tokio::test]
    async fn test_orchestrator_with_progress() {
        let handler = LoggingHandler;
        let orchestrator = PipelineOrchestrator::new(Some(handler));
        assert!(orchestrator.progress_handler.is_some());
    }
}
