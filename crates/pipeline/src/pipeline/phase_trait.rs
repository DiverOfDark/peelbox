use super::context::AnalysisContext;
use super::service_context::ServiceContext;
use anyhow::Result;
use async_trait::async_trait;

/// Phases iterate registry implementations. Detection mode is handled by registry registration order.
#[async_trait]
pub trait WorkflowPhase: Send + Sync {
    fn name(&self) -> &'static str;

    async fn execute(&self, context: &mut AnalysisContext) -> Result<()>;
}

/// Phases iterate registry implementations. Detection mode is handled by registry registration order.
#[async_trait]
pub trait ServicePhase: Send + Sync {
    fn name(&self) -> &'static str;

    async fn execute(&self, context: &mut ServiceContext) -> Result<()>;
}
