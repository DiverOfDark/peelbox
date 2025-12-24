use super::service_analysis::Service;
use crate::pipeline::Confidence;
use crate::stack::StackRegistry;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    pub build_cmd: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub confidence: Confidence,
}

fn try_deterministic(
    service: &Service,
    stack_registry: &Arc<StackRegistry>,
    repo_path: &std::path::Path,
) -> Option<BuildInfo> {
    let build_system = stack_registry.get_build_system(service.build_system.clone())?;

    let wolfi_index = crate::validation::WolfiPackageIndex::fetch().ok()?;

    // Read manifest content to pass to build_template for version parsing
    let manifest_path = repo_path.join(&service.path).join(&service.manifest);
    let manifest_content = std::fs::read_to_string(&manifest_path).ok();

    let template = build_system.build_template(&wolfi_index, manifest_content.as_deref());

    let build_cmd = template.build_commands.first().cloned();
    let output_dir = template.artifacts.first().map(|artifact| {
        let path = artifact
            .replace("/{project_name}", "")
            .replace("{project_name}", "")
            .trim_end_matches('/')
            .to_string();

        if path.contains('*') {
            PathBuf::from(&path)
                .parent()
                .unwrap_or(&PathBuf::from(&path))
                .to_path_buf()
        } else {
            PathBuf::from(path)
        }
    });

    Some(BuildInfo {
        build_cmd,
        output_dir,
        confidence: Confidence::High,
    })
}

use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use async_trait::async_trait;

pub struct BuildPhase;

#[async_trait]
impl ServicePhase for BuildPhase {
    fn name(&self) -> &'static str {
        "BuildPhase"
    }

    async fn execute(&self, context: &mut ServiceContext) -> Result<()> {
        if let Some(deterministic) = try_deterministic(&context.service, context.stack_registry(), context.repo_path()) {
            context.build = Some(deterministic);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_cargo() {
        // Tests use mock instead of fetching real APKINDEX
        // In production, WolfiPackageIndex::fetch() is called inside try_deterministic
        // For testing, we verify that the build system returns correct template
        let stack_registry = Arc::new(StackRegistry::with_defaults(None));
        let wolfi_index = crate::validation::WolfiPackageIndex::for_tests();

        let build_system = stack_registry
            .get_build_system(crate::stack::BuildSystemId::Cargo)
            .unwrap();

        let template = build_system.build_template(&wolfi_index, None);

        assert_eq!(template.build_commands.first(), Some(&"cargo build --release".to_string()));
        assert!(!template.artifacts.is_empty());
    }

    #[test]
    fn test_deterministic_maven() {
        let stack_registry = Arc::new(StackRegistry::with_defaults(None));
        let wolfi_index = crate::validation::WolfiPackageIndex::for_tests();

        let build_system = stack_registry
            .get_build_system(crate::stack::BuildSystemId::Maven)
            .unwrap();

        let template = build_system.build_template(&wolfi_index, None);

        assert_eq!(
            template.build_commands.first(),
            Some(&"mvn clean package -DskipTests".to_string())
        );
        assert!(!template.artifacts.is_empty());
    }
}
