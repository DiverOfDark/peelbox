use super::runtime::RuntimeInfo;
use super::scan::ScanResult;
use super::structure::Service;
use crate::extractors::health::HealthCheckExtractor;
use crate::fs::RealFileSystem;
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use crate::pipeline::Confidence;
use crate::stack::StackRegistry;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthInfo {
    pub health_endpoints: Vec<HealthEndpoint>,
    pub recommended_liveness: Option<String>,
    pub recommended_readiness: Option<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthEndpoint {
    pub path: String,
    pub method: String,
}

fn build_prompt(service: &Service, runtime: &RuntimeInfo, extracted: &[String]) -> String {
    format!(
        r#"Detect health check endpoints for this service.

Service path: {}
Runtime: {}
Framework: {}

Extracted endpoints from code: {}

Respond with JSON:
{{
  "health_endpoints": [
    {{"path": "/health", "method": "GET"}},
    {{"path": "/api/health", "method": "GET"}}
  ],
  "recommended_liveness": "/health" | null,
  "recommended_readiness": "/api/ready" | null,
  "confidence": "high" | "medium" | "low"
}}

Rules:
- health_endpoints: All detected health/status endpoints
- recommended_liveness: Lightweight check for container liveness
- recommended_readiness: Full readiness check (DB connections, etc.)
- Use framework defaults if no explicit endpoints found
"#,
        service.path.display(),
        runtime.runtime,
        runtime.framework.as_deref().unwrap_or("unknown"),
        if extracted.is_empty() {
            "None found".to_string()
        } else {
            extracted.join(", ")
        }
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    service: &Service,
    runtime: &RuntimeInfo,
    scan: &ScanResult,
    stack_registry: &Arc<StackRegistry>,
    logger: &Arc<HeuristicLogger>,
) -> Result<HealthInfo> {
    let context = super::extractor_helper::create_service_context(scan, service);
    let extractor = HealthCheckExtractor::new(RealFileSystem);
    let extracted_info = extractor.extract(&context);
    let extracted: Vec<String> = extracted_info
        .iter()
        .map(|info| info.endpoint.clone())
        .collect();

    if !extracted.is_empty() {
        let health_endpoints: Vec<HealthEndpoint> = extracted
            .into_iter()
            .map(|path| HealthEndpoint {
                path,
                method: "GET".to_string(),
            })
            .collect();

        let recommended = health_endpoints.first().map(|e| e.path.clone());

        return Ok(HealthInfo {
            health_endpoints,
            recommended_liveness: recommended.clone(),
            recommended_readiness: recommended,
            confidence: Confidence::High,
        });
    }

    if let Some(framework_default) = try_framework_defaults(runtime, stack_registry) {
        return Ok(framework_default);
    }

    let prompt = build_prompt(service, runtime, &extracted);
    super::llm_helper::query_llm_with_logging(llm_client, prompt, 500, "health", logger).await
}

fn try_framework_defaults(
    runtime: &RuntimeInfo,
    stack_registry: &Arc<crate::stack::StackRegistry>,
) -> Option<HealthInfo> {
    let framework_name = runtime.framework.as_deref()?;

    for fw_id in crate::stack::FrameworkId::all_variants() {
        if let Some(fw) = stack_registry.get_framework(*fw_id) {
            if fw.id().name() == framework_name {
                let endpoints = fw.health_endpoints();
                if endpoints.is_empty() {
                    return None;
                }

                let health_endpoints: Vec<HealthEndpoint> = endpoints
                    .iter()
                    .map(|path| HealthEndpoint {
                        path: path.to_string(),
                        method: "GET".to_string(),
                    })
                    .collect();

                let recommended = endpoints.first().map(|e| e.to_string());

                return Some(HealthInfo {
                    health_endpoints,
                    recommended_liveness: recommended.clone(),
                    recommended_readiness: recommended,
                    confidence: Confidence::High,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_framework_defaults_spring() {
        let runtime = RuntimeInfo {
            runtime: "java".to_string(),
            runtime_version: None,
            framework: Some("Spring Boot".to_string()),
            confidence: crate::pipeline::Confidence::High,
        };

        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());
        let result = try_framework_defaults(&runtime, &stack_registry).unwrap();
        assert_eq!(result.health_endpoints.len(), 3);
        assert_eq!(result.health_endpoints[0].path, "/actuator/health");
        assert_eq!(
            result.recommended_liveness,
            Some("/actuator/health".to_string())
        );
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_framework_defaults_express() {
        let runtime = RuntimeInfo {
            runtime: "node".to_string(),
            runtime_version: None,
            framework: Some("Express".to_string()),
            confidence: crate::pipeline::Confidence::High,
        };

        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());
        let result = try_framework_defaults(&runtime, &stack_registry).unwrap();
        assert!(!result.health_endpoints.is_empty());
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let runtime = RuntimeInfo {
            runtime: "node".to_string(),
            runtime_version: None,
            framework: Some("express".to_string()),
            confidence: crate::pipeline::Confidence::High,
        };

        let extracted = vec!["/health".to_string(), "/api/status".to_string()];
        let prompt = build_prompt(&service, &runtime, &extracted);

        assert!(prompt.contains("apps/api"));
        assert!(prompt.contains("express"));
        assert!(prompt.contains("/health"));
    }
}
