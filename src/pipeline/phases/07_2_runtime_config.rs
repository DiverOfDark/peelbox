use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use anyhow::Result;
use async_trait::async_trait;

pub struct RuntimeConfigPhase;

#[async_trait]
impl ServicePhase for RuntimeConfigPhase {
    fn name(&self) -> &'static str {
        "RuntimeConfigPhase"
    }

    async fn execute(&self, context: &mut ServiceContext) -> Result<()> {
        let stack = context
            .stack
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Stack must be detected before RuntimeConfigPhase"))?;

        let stack_registry = context.stack_registry();
        let runtime = stack_registry.get_runtime(stack.runtime.clone(), None);

        let scan = context.scan()?;
        let files = &scan.file_tree;

        let framework = stack
            .framework
            .as_ref()
            .and_then(|fw_id| stack_registry.get_framework(fw_id.clone()));

        if let Some(config) = runtime.try_extract(files, framework) {
            context.runtime_config = Some(config);
        }

        Ok(())
    }
}
