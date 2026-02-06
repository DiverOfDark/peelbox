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
        let repo_path = context.repo_path();
        let service_path = &context.service.path;

        // Convert relative paths to absolute paths, filtering to service scope
        let absolute_files: Vec<std::path::PathBuf> = scan
            .file_tree
            .iter()
            .filter(|p| {
                let sp = service_path.as_path();
                sp == std::path::Path::new(".") || sp == std::path::Path::new("") || p.starts_with(sp)
            })
            .map(|p| repo_path.join(p))
            .collect();

        let framework = stack
            .framework
            .as_ref()
            .and_then(|fw_id| stack_registry.get_framework(fw_id.clone()));

        if let Some(config) = runtime.try_extract(&absolute_files, framework) {
            context.runtime_config = Some(config);
        }

        Ok(())
    }
}
