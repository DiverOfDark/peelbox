use super::scan::ScanResult;
use super::structure::Service;
use crate::extractors::port::PortExtractor;
use crate::fs::RealFileSystem;
use crate::languages::LanguageRegistry;
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use anyhow::Result;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: Option<u16>,
    pub from_env: bool,
    pub env_var: Option<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
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
        service.build_system,
        service.language,
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

pub async fn execute(
    llm_client: &dyn LLMClient,
    service: &Service,
    scan: &ScanResult,
    registry: &LanguageRegistry,
    logger: &Arc<HeuristicLogger>,
) -> Result<PortInfo> {
    let context = super::extractor_helper::create_service_context(scan, service);
    let extractor = PortExtractor::with_registry(RealFileSystem, registry.clone());
    let extracted_info = extractor.extract(&context);
    let extracted: Vec<u16> = extracted_info.iter().map(|info| info.port).collect();

    if !extracted.is_empty() {
        let port = extracted[0];
        return Ok(PortInfo {
            port: Some(port),
            from_env: false,
            env_var: None,
            confidence: Confidence::High,
        });
    }

    if let Some(deterministic) = try_deterministic(service) {
        return Ok(deterministic);
    }

    let prompt = build_prompt(service, &extracted);
    super::llm_helper::query_llm_with_logging(llm_client, prompt, 300, "port", logger).await
}

fn try_deterministic(service: &Service) -> Option<PortInfo> {
    let registry = LanguageRegistry::with_defaults();
    let language_def = registry.get_language(&service.language)?;

    let default_port = language_def.default_port()?;

    Some(PortInfo {
        port: Some(default_port),
        from_env: true,
        env_var: Some("PORT".to_string()),
        confidence: Confidence::Medium,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_deterministic_node() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let result = try_deterministic(&service).unwrap();
        assert_eq!(result.port, Some(3000));
        assert!(result.from_env);
        assert_eq!(result.env_var, Some("PORT".to_string()));
    }

    #[test]
    fn test_deterministic_java() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "pom.xml".to_string(),
            language: "Java".to_string(),
            build_system: "maven".to_string(),
        };

        let result = try_deterministic(&service).unwrap();
        assert_eq!(result.port, Some(8080));
    }

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/api"),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let ports = vec![3000, 8080];
        let prompt = build_prompt(&service, &ports);

        assert!(prompt.contains("apps/api"));
        assert!(prompt.contains("3000"));
        assert!(prompt.contains("8080"));
    }
}
