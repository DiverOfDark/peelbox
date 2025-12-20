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

    fn try_deterministic(&self, context: &mut ServiceContext) -> Result<Option<()>> {
        let runtime_info = context
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Runtime must be detected before RuntimeConfigPhase"))?;

        let stack_registry = context.stack_registry();
        let runtime = stack_registry.get_runtime(runtime_info.runtime);

        let scan = context.scan()?;
        let files = &scan.file_tree;

        let framework = runtime_info.framework.as_ref().and_then(|fw_name| {
            for fw_id in crate::stack::FrameworkId::all_variants() {
                if let Some(fw) = stack_registry.get_framework(*fw_id) {
                    if fw.id().name() == fw_name {
                        return Some(fw as &dyn crate::stack::framework::Framework);
                    }
                }
            }
            None
        });

        if let Some(config) = runtime.try_extract(files, framework) {
            context.runtime_config = Some(config);
            return Ok(Some(()));
        }

        Ok(None)
    }

    async fn execute_llm(&self, _context: &mut ServiceContext) -> Result<()> {
        // LLM fallback not yet implemented - this is in technical debt
        // For now, return empty config if deterministic extraction failed
        Ok(())
    }
}
