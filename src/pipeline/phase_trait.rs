use super::context::AnalysisContext;
use super::service_context::ServiceContext;
use crate::config::DetectionMode;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait WorkflowPhase: Send + Sync {
    fn name(&self) -> &'static str;

    /// Try deterministic detection (used in StaticOnly mode, and as first attempt in Full mode)
    /// Returns Ok(Some(())) if deterministic detection succeeded
    /// Returns Ok(None) if no deterministic detection is available
    /// Returns Err(e) if deterministic detection failed
    fn try_deterministic(&self, _context: &mut AnalysisContext) -> Result<Option<()>> {
        Ok(None)
    }

    /// LLM-based detection (used when deterministic fails in Full/LLMOnly mode)
    async fn execute_llm(&self, context: &mut AnalysisContext) -> Result<()>;

    /// Execute phase with mode-aware logic (deterministic first, then LLM if needed)
    async fn execute(&self, context: &mut AnalysisContext) -> Result<()> {
        // Try deterministic first (always, regardless of mode)
        if self.try_deterministic(context)?.is_some() {
            return Ok(());
        }

        // If no deterministic detection, check mode
        match context.detection_mode {
            DetectionMode::StaticOnly => {
                anyhow::bail!(
                    "Phase {} has no deterministic detection available (StaticOnly mode)",
                    self.name()
                )
            }
            DetectionMode::LLMOnly | DetectionMode::Full => self.execute_llm(context).await,
        }
    }
}

#[async_trait]
pub trait ServicePhase: Send + Sync {
    fn name(&self) -> &'static str;

    /// Try deterministic detection (used in StaticOnly mode, and as first attempt in Full mode)
    /// Returns Ok(Some(())) if deterministic detection succeeded
    /// Returns Ok(None) if no deterministic detection is available
    /// Returns Err(e) if deterministic detection failed
    fn try_deterministic(&self, _context: &mut ServiceContext) -> Result<Option<()>> {
        Ok(None)
    }

    /// LLM-based detection (used when deterministic fails in Full/LLMOnly mode)
    async fn execute_llm(&self, context: &mut ServiceContext) -> Result<()>;

    /// Execute phase with mode-aware logic (deterministic first, then LLM if needed)
    async fn execute(&self, context: &mut ServiceContext) -> Result<()> {
        // Try deterministic first (always, regardless of mode)
        if self.try_deterministic(context)?.is_some() {
            return Ok(());
        }

        // If no deterministic detection, check mode
        match context.analysis_context.detection_mode {
            DetectionMode::StaticOnly => {
                anyhow::bail!(
                    "Service phase {} has no deterministic detection available (StaticOnly mode)",
                    self.name()
                )
            }
            DetectionMode::LLMOnly | DetectionMode::Full => self.execute_llm(context).await,
        }
    }
}
