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

        let workflow_phases: Vec<(Box<dyn WorkflowPhase>, &str)> = vec![
            (Box::new(ScanPhase), "ScanPhase"),
            (Box::new(ClassifyPhase), "ClassifyPhase"),
            (Box::new(StructurePhase), "StructurePhase"),
            (Box::new(DependenciesPhase), "DependenciesPhase"),
            (Box::new(BuildOrderPhase), "BuildOrderPhase"),
            (Box::new(RootCachePhase), "RootCachePhase"),
            (Box::new(ServiceAnalysisPhase), "ServiceAnalysisPhase"),
            (Box::new(AssemblePhase), "AssemblePhase"),
        ];

        for (phase, phase_name) in workflow_phases {
            info!("Phase: {}", phase_name);

            if let Some(handler) = &self.progress_handler {
                handler.on_progress(&ProgressEvent::PhaseStarted {
                    phase: phase_name.to_string(),
                });
            }

            let phase_start = Instant::now();
            phase
                .execute(context)
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

        Ok(context.builds.clone())
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
