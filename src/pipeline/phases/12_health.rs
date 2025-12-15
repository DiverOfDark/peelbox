use super::runtime::RuntimeInfo;
use super::scan::ScanResult;
use super::structure::Service;
use crate::extractors::health::HealthCheckExtractor;
use crate::fs::RealFileSystem;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
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
    registry: &LanguageRegistry,
) -> Result<HealthInfo> {
    let context = super::extractor_helper::create_service_context(scan, service);
    let extractor = HealthCheckExtractor::with_registry(RealFileSystem, registry.clone());
    let extracted_info = extractor.extract(&context);
    let extracted: Vec<String> = extracted_info.iter().map(|info| info.endpoint.clone()).collect();

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

    if let Some(framework_default) = try_framework_defaults(runtime) {
        return Ok(framework_default);
    }

    let prompt = build_prompt(service, runtime, &extracted);
    super::llm_helper::query_llm(llm_client, prompt, 500, "health check detection").await
}

fn try_framework_defaults(runtime: &RuntimeInfo) -> Option<HealthInfo> {
    let framework = runtime.framework.as_deref()?;

    let endpoint = match framework {
        "spring-boot" => "/actuator/health",
        "aspnet" => "/health",
        _ => return None,
    };

    Some(HealthInfo {
        health_endpoints: vec![HealthEndpoint {
            path: endpoint.to_string(),
            method: "GET".to_string(),
        }],
        recommended_liveness: Some(endpoint.to_string()),
        recommended_readiness: Some(endpoint.to_string()),
        confidence: Confidence::High,
    })
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
            framework: Some("spring-boot".to_string()),
            confidence: super::super::runtime::Confidence::High,
        };

        let result = try_framework_defaults(&runtime).unwrap();
        assert_eq!(result.health_endpoints.len(), 1);
        assert_eq!(result.health_endpoints[0].path, "/actuator/health");
        assert_eq!(result.recommended_liveness, Some("/actuator/health".to_string()));
    }

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let runtime = RuntimeInfo {
            runtime: "node".to_string(),
            runtime_version: None,
            framework: Some("express".to_string()),
            confidence: super::super::runtime::Confidence::High,
        };

        let extracted = vec!["/health".to_string(), "/api/status".to_string()];
        let prompt = build_prompt(&service, &runtime, &extracted);

        assert!(prompt.contains("apps/api"));
        assert!(prompt.contains("express"));
        assert!(prompt.contains("/health"));
    }
}
