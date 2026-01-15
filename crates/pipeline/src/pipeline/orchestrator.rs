use super::context::AnalysisContext;
use super::phase_trait::WorkflowPhase;
use super::phases::{
    assemble::AssemblePhase, root_cache::RootCachePhase, scan::ScanPhase,
    service_analysis::ServiceAnalysisPhase, workspace::WorkspaceStructurePhase,
};
use anyhow::{Context, Result};
use peelbox_core::output::schema::UniversalBuild;
use std::path::Path;
use std::time::Instant;
use tracing::info;

pub struct PipelineOrchestrator;

impl Default for PipelineOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineOrchestrator {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        repo_path: &Path,
        context: &mut AnalysisContext,
    ) -> Result<Vec<UniversalBuild>> {
        let start = Instant::now();
        info!(
            repo = %repo_path.display(),
            "Starting detection pipeline"
        );

        let workflow_phases: Vec<Box<dyn WorkflowPhase>> = vec![
            Box::new(ScanPhase),
            Box::new(WorkspaceStructurePhase),
            Box::new(RootCachePhase),
            Box::new(ServiceAnalysisPhase),
            Box::new(AssemblePhase),
        ];

        for phase in workflow_phases {
            let phase_name = phase.name();
            info!(phase = %phase_name, "Starting phase");

            let phase_start = Instant::now();
            phase
                .execute(context)
                .await
                .with_context(|| format!("Phase {} failed", phase_name))?;

            info!(
                phase = %phase_name,
                duration_ms = phase_start.elapsed().as_millis(),
                "Phase complete"
            );
        }

        info!(
            projects_detected = context.builds.len(),
            total_time_ms = start.elapsed().as_millis(),
            "Detection complete"
        );

        Ok(std::mem::take(&mut context.builds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let _orchestrator = PipelineOrchestrator::new();
    }
}
