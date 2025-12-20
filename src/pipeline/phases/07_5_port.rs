use super::runtime::RuntimeInfo;
use super::structure::Service;
use crate::extractors::port::PortExtractor;
use crate::fs::RealFileSystem;
use crate::pipeline::Confidence;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: Option<u16>,
    pub from_env: bool,
    pub env_var: Option<String>,
    pub confidence: Confidence,
}

fn build_prompt(service: &Service, extracted_ports: &[u16]) -> String {
    format!(
        r#"Detect the port this service listens on.

Service path: {}
Build system: {}
Language: {}

Extracted ports from code/config: {}

Respond with JSON:
{{
  "port": 3000 | null,
  "from_env": true | false,
  "env_var": "PORT" | "HTTP_PORT" | null,
  "confidence": "high" | "medium" | "low"
}}

Rules:
- port: Numeric port if hardcoded, null if dynamic
- from_env: true if port comes from environment variable
- env_var: Name of environment variable (if from_env is true)
- Default to 3000 for Node.js, 8080 for Java/Spring, 8000 for Python if unclear
"#,
        service.path.display(),
        service.build_system.name(),
        service.language.name(),
        if extracted_ports.is_empty() {
            "None found".to_string()
        } else {
            extracted_ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    )
}

fn try_framework_defaults(
    runtime: &RuntimeInfo,
    stack_registry: &Arc<crate::stack::StackRegistry>,
) -> Option<PortInfo> {
    let framework_name = runtime.framework.as_deref()?;

    for fw_id in crate::stack::FrameworkId::all_variants() {
        if let Some(fw) = stack_registry.get_framework(*fw_id) {
            if fw.id().name() == framework_name {
                let ports = fw.default_ports();
                if !ports.is_empty() {
                    return Some(PortInfo {
                        port: Some(ports[0]),
                        from_env: true,
                        env_var: Some("PORT".to_string()),
                        confidence: Confidence::High,
                    });
                }
            }
        }
    }
    None
}

fn try_deterministic(
    service: &Service,
    stack_registry: &Arc<crate::stack::StackRegistry>,
) -> Option<PortInfo> {
    let language_def = stack_registry.get_language(service.language)?;

    let default_port = language_def.default_port()?;

    Some(PortInfo {
        port: Some(default_port),
        from_env: true,
        env_var: Some("PORT".to_string()),
        confidence: Confidence::Medium,
    })
}

use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use async_trait::async_trait;

pub struct PortPhase;

#[async_trait]
impl ServicePhase for PortPhase {
    fn name(&self) -> &'static str {
        "PortPhase"
    }

    fn try_deterministic(&self, context: &mut ServiceContext) -> Result<Option<()>> {
        let runtime = context
            .runtime
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Runtime must be available before port detection"))?;

        let scan = context.scan()?;
        let extractor_context = crate::extractors::context::ServiceContext {
            path: scan.repo_path.join(&context.service.path),
            language: Some(context.service.language),
            build_system: Some(context.service.build_system),
        };
        let extractor = PortExtractor::new(RealFileSystem);
        let extracted_info = extractor.extract(&extractor_context);
        let extracted: Vec<u16> = extracted_info.iter().map(|info| info.port).collect();

        if !extracted.is_empty() {
            let port = extracted[0];
            context.port = Some(PortInfo {
                port: Some(port),
                from_env: false,
                env_var: None,
                confidence: Confidence::High,
            });
            Ok(Some(()))
        } else if let Some(framework_default) =
            try_framework_defaults(runtime, context.stack_registry())
        {
            context.port = Some(framework_default);
            Ok(Some(()))
        } else if let Some(deterministic) =
            try_deterministic(&context.service, context.stack_registry())
        {
            context.port = Some(deterministic);
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    async fn execute_llm(&self, context: &mut ServiceContext) -> Result<()> {
        let scan = context.scan()?;
        let extractor_context = crate::extractors::context::ServiceContext {
            path: scan.repo_path.join(&context.service.path),
            language: Some(context.service.language),
            build_system: Some(context.service.build_system),
        };
        let extractor = PortExtractor::new(RealFileSystem);
        let extracted_info = extractor.extract(&extractor_context);
        let extracted: Vec<u16> = extracted_info.iter().map(|info| info.port).collect();

        let prompt = build_prompt(&context.service, &extracted);
        let result = super::llm_helper::query_llm_with_logging(
            context.llm_client(),
            prompt,
            300,
            "port",
            context.heuristic_logger(),
        )
        .await?;

        context.port = Some(result);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_framework_defaults_spring() {
        let runtime = RuntimeInfo {
            runtime: crate::stack::RuntimeId::JVM,
            runtime_version: None,
            framework: Some("Spring Boot".to_string()),
            confidence: crate::pipeline::Confidence::High,
        };

        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());
        let result = try_framework_defaults(&runtime, &stack_registry).unwrap();
        assert_eq!(result.port, Some(8080));
        assert!(result.from_env);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_framework_defaults_express() {
        let runtime = RuntimeInfo {
            runtime: crate::stack::RuntimeId::Node,
            runtime_version: None,
            framework: Some("Express".to_string()),
            confidence: crate::pipeline::Confidence::High,
        };

        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());
        let result = try_framework_defaults(&runtime, &stack_registry).unwrap();
        assert_eq!(result.port, Some(3000));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_deterministic_node() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());
        let result = try_deterministic(&service, &stack_registry).unwrap();
        assert_eq!(result.port, Some(3000));
        assert!(result.from_env);
        assert_eq!(result.env_var, Some("PORT".to_string()));
    }

    #[test]
    fn test_deterministic_java() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "pom.xml".to_string(),
            language: crate::stack::LanguageId::Java,
            build_system: crate::stack::BuildSystemId::Maven,
        };

        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());
        let result = try_deterministic(&service, &stack_registry).unwrap();
        assert_eq!(result.port, Some(8080));
    }

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let ports = vec![3000, 8080];
        let prompt = build_prompt(&service, &ports);

        assert!(prompt.contains("apps/api"));
        assert!(prompt.contains("3000"));
        assert!(prompt.contains("8080"));
    }
}
