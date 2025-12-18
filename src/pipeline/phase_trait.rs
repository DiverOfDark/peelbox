use super::context::AnalysisContext;
use super::service_context::ServiceContext;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait WorkflowPhase: Send + Sync {
    fn name(&self) -> &'static str;
    async fn execute(&self, context: &mut AnalysisContext) -> Result<()>;
}

#[async_trait]
pub trait ServicePhase: Send + Sync {
    type Output;
    async fn execute(&self, context: &ServiceContext<'_>) -> Result<Self::Output>;
}
