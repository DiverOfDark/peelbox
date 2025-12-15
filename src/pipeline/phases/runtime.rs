use super::scan::ScanResult;
use super::structure::Service;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub runtime: String,
    pub runtime_version: Option<String>,
    pub framework: Option<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

fn build_prompt(service: &Service, files: &[PathBuf], manifest_excerpt: Option<&str>) -> String {
    let file_list: Vec<String> = files
        .iter()
        .take(20)
        .map(|p| p.display().to_string())
        .collect();

    format!(
        r#"Detect the runtime and framework for this service.

Service path: {}
Build system: {}
Language: {}

Files in service:
{}

Manifest excerpt:
{}

Respond with JSON:
{{
  "runtime": "node" | "python" | "rust" | "java" | "go" | "dotnet" | "ruby" | "php" | "static" | "unknown",
  "runtime_version": "18.0.0" | null,
  "framework": "nextjs" | "express" | "fastapi" | "spring-boot" | "gin" | "aspnet" | "rails" | "laravel" | "none" | "unknown" | null,
  "confidence": "high" | "medium" | "low"
}}

Rules:
- runtime: The language/runtime environment
- runtime_version: Specific version if detected from manifest
- framework: Web framework or major library (null if not applicable)
- confidence: Based on clarity of indicators
"#,
        service.path.display(),
        service.build_system,
        service.language,
        file_list.join("\n"),
        manifest_excerpt.unwrap_or("None")
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    service: &Service,
    scan: &ScanResult,
) -> Result<RuntimeInfo> {
    if let Some(deterministic) = try_deterministic(service) {
        return Ok(deterministic);
    }

    let files = extract_relevant_files(scan, service);
    let manifest_excerpt = extract_manifest_excerpt(scan, service)?;

    let prompt = build_prompt(service, &files, manifest_excerpt.as_deref());

    let request = crate::llm::types::ChatRequest {
        messages: vec![crate::llm::types::Message {
            role: "user".to_string(),
            content: prompt,
        }],
        temperature: Some(0.1),
        max_tokens: Some(500),
    };

    let response = llm_client
        .chat(request)
        .await
        .context("Failed to call LLM for runtime detection")?;

    let runtime_info: RuntimeInfo = serde_json::from_str(&response.content)
        .context("Failed to parse runtime detection response")?;

    Ok(runtime_info)
}

fn try_deterministic(service: &Service) -> Option<RuntimeInfo> {
    let registry = LanguageRegistry::new();
    let language_def = registry.get_by_name(&service.language)?;

    let runtime = language_def.runtime_name()?;

    Some(RuntimeInfo {
        runtime: runtime.to_string(),
        runtime_version: None,
        framework: None,
        confidence: Confidence::High,
    })
}

fn extract_relevant_files(scan: &ScanResult, service: &Service) -> Vec<PathBuf> {
    let service_dir = &service.path;

    scan.file_tree
        .iter()
        .filter(|p| p.starts_with(service_dir))
        .take(50)
        .cloned()
        .collect()
}

fn extract_manifest_excerpt(scan: &ScanResult, service: &Service) -> Result<Option<String>> {
    let manifest_path = scan.repo_path.join(&service.path).join(&service.manifest);

    if !manifest_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

    let excerpt = if content.len() > 500 {
        format!("{}...", &content[..500])
    } else {
        content
    };

    Ok(Some(excerpt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_rust() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
        };

        let result = try_deterministic(&service).unwrap();
        assert_eq!(result.runtime, "rust");
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_deterministic_node() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let result = try_deterministic(&service).unwrap();
        assert_eq!(result.runtime, "node");
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let files = vec![
            PathBuf::from("apps/web/package.json"),
            PathBuf::from("apps/web/next.config.js"),
        ];

        let prompt = build_prompt(&service, &files, Some(r#"{"name": "web"}"#));

        assert!(prompt.contains("apps/web"));
        assert!(prompt.contains("npm"));
        assert!(prompt.contains("next.config.js"));
    }
}
