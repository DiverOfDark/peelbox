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

fn try_deterministic(service: &Service, stack_registry: &Arc<StackRegistry>) -> Option<BuildInfo> {
    let build_system = stack_registry.get_build_system(service.build_system.clone())?;

    let template = build_system.build_template();

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
        if let Some(deterministic) = try_deterministic(&context.service, context.stack_registry()) {
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
        let stack_registry = Arc::new(StackRegistry::with_defaults(None));
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: crate::stack::LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Cargo,
        };

        let result = try_deterministic(&service, &stack_registry).unwrap();
        assert_eq!(result.build_cmd, Some("cargo build --release".to_string()));
        assert_eq!(result.output_dir, Some(PathBuf::from("target/release")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_deterministic_maven() {
        let stack_registry = Arc::new(StackRegistry::with_defaults(None));
        let service = Service {
            path: PathBuf::from("."),
            manifest: "pom.xml".to_string(),
            language: crate::stack::LanguageId::Java,
            build_system: crate::stack::BuildSystemId::Maven,
        };

        let result = try_deterministic(&service, &stack_registry).unwrap();
        assert_eq!(
            result.build_cmd,
            Some("mvn clean package -DskipTests".to_string())
        );
        assert_eq!(result.output_dir, Some(PathBuf::from("target")));
    }
}
