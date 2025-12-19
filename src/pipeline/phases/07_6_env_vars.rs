use super::structure::Service;
use crate::extractors::env_vars::EnvVarExtractor;
use crate::fs::RealFileSystem;
use crate::pipeline::Confidence;
use anyhow::Result;
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
        service.build_system.name(),
        service.language.name(),
        if extracted_vars.is_empty() {
            "None found".to_string()
        } else {
            extracted_vars.join(", ")
        }
    )
}

use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use async_trait::async_trait;

pub struct EnvVarsPhase;

#[async_trait]
impl ServicePhase for EnvVarsPhase {
    fn name(&self) -> &'static str {
        "EnvVarsPhase"
    }

    type Output = EnvVarsInfo;

    async fn execute(&self, context: &ServiceContext) -> Result<EnvVarsInfo> {
        let service_context =
            super::extractor_helper::create_service_context(context.scan()?, context.service);
        let extractor = EnvVarExtractor::new(RealFileSystem);
        let extracted_info = extractor.extract(&service_context);
        let extracted: Vec<String> = extracted_info
            .iter()
            .map(|info| info.name.clone())
            .collect();

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

            let result = EnvVarsInfo {
                env_vars,
                confidence: Confidence::High,
            };
            return Ok(result);
        }

        let prompt = build_prompt(context.service, &extracted);
        let result: EnvVarsInfo = super::llm_helper::query_llm_with_logging(
            context.llm_client(),
            prompt,
            800,
            "env_vars",
            context.heuristic_logger(),
        )
        .await?;
        Ok(result)
    }
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
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let vars = vec!["DATABASE_URL".to_string(), "PORT".to_string()];
        let prompt = build_prompt(&service, &vars);

        assert!(prompt.contains("apps/api"));
        assert!(prompt.contains("DATABASE_URL"));
        assert!(prompt.contains("PORT"));
    }
}
