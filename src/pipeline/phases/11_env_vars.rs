use super::scan::ScanResult;
use super::structure::Service;
use crate::extractors::env_vars::EnvVarExtractor;
use crate::fs::RealFileSystem;
use crate::languages::LanguageRegistry;
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use anyhow::Result;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarsInfo {
    pub env_vars: Vec<EnvVar>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

fn build_prompt(service: &Service, extracted_vars: &[String]) -> String {
    format!(
        r#"Detect environment variables required by this service.

Service path: {}
Build system: {}
Language: {}

Extracted env vars from .env.example and code: {}

Respond with JSON:
{{
  "env_vars": [
    {{"name": "DATABASE_URL", "required": true, "default_value": null, "description": "PostgreSQL connection string"}},
    {{"name": "PORT", "required": false, "default_value": "3000", "description": "HTTP port"}}
  ],
  "confidence": "high" | "medium" | "low"
}}

Rules:
- Include only application-level env vars (not build-time like NODE_ENV)
- required: true if app will fail without it
- default_value: Value used if not provided
"#,
        service.path.display(),
        service.build_system,
        service.language,
        if extracted_vars.is_empty() {
            "None found".to_string()
        } else {
            extracted_vars.join(", ")
        }
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    service: &Service,
    scan: &ScanResult,
    registry: &LanguageRegistry,
    logger: &Arc<HeuristicLogger>,
) -> Result<EnvVarsInfo> {
    let context = super::extractor_helper::create_service_context(scan, service);
    let extractor = EnvVarExtractor::with_registry(RealFileSystem, registry.clone());
    let extracted_info = extractor.extract(&context);
    let extracted: Vec<String> = extracted_info.iter().map(|info| info.name.clone()).collect();

    if !extracted.is_empty() {
        let env_vars: Vec<EnvVar> = extracted
            .into_iter()
            .map(|name| EnvVar {
                name,
                required: true,
                default_value: None,
                description: None,
            })
            .collect();

        return Ok(EnvVarsInfo {
            env_vars,
            confidence: Confidence::High,
        });
    }

    let prompt = build_prompt(service, &extracted);
    super::llm_helper::query_llm_with_logging(llm_client, prompt, 800, "env_vars", logger).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let vars = vec!["DATABASE_URL".to_string(), "PORT".to_string()];
        let prompt = build_prompt(&service, &vars);

        assert!(prompt.contains("apps/api"));
        assert!(prompt.contains("DATABASE_URL"));
        assert!(prompt.contains("PORT"));
    }
}
