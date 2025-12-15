use super::scan::ScanResult;
use super::structure::Service;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrypointInfo {
    pub entrypoint: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

fn build_prompt(service: &Service, manifest_excerpt: Option<&str>) -> String {
    format!(
        r#"Detect the entrypoint for this service.

Service path: {}
Build system: {}
Language: {}

Manifest excerpt:
{}

Respond with JSON:
{{
  "entrypoint": "./server.js" | "./target/release/app" | "python main.py" | "java -jar app.jar",
  "confidence": "high" | "medium" | "low"
}}

Rules:
- entrypoint: Command or file path to start the service
- Must be executable/runnable in container context
- Include interpreter if needed (python, node, java)
"#,
        service.path.display(),
        service.build_system,
        service.language,
        manifest_excerpt.unwrap_or("None")
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    service: &Service,
    scan: &ScanResult,
) -> Result<EntrypointInfo> {
    if let Some(deterministic) = try_deterministic(service, scan)? {
        return Ok(deterministic);
    }

    let manifest_excerpt = extract_manifest_excerpt(scan, service)?;

    let prompt = build_prompt(service, manifest_excerpt.as_deref());

    let request = crate::llm::types::ChatRequest {
        messages: vec![crate::llm::types::Message {
            role: "user".to_string(),
            content: prompt,
        }],
        temperature: Some(0.1),
        max_tokens: Some(300),
    };

    let response = llm_client
        .chat(request)
        .await
        .context("Failed to call LLM for entrypoint detection")?;

    let entrypoint_info: EntrypointInfo = serde_json::from_str(&response.content)
        .context("Failed to parse entrypoint detection response")?;

    Ok(entrypoint_info)
}

fn try_deterministic(service: &Service, scan: &ScanResult) -> Result<Option<EntrypointInfo>> {
    let registry = LanguageRegistry::new();
    let language_def = match registry.get_by_name(&service.language) {
        Some(def) => def,
        None => return Ok(None),
    };

    let manifest_path = scan.repo_path.join(&service.path).join(&service.manifest);

    if manifest_path.exists() {
        let content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

        if let Some(entrypoint) = language_def.parse_entrypoint_from_manifest(&content) {
            return Ok(Some(EntrypointInfo {
                entrypoint,
                confidence: Confidence::High,
            }));
        }
    }

    if let Some(entrypoint) = language_def.default_entrypoint(&service.build_system) {
        return Ok(Some(EntrypointInfo {
            entrypoint,
            confidence: Confidence::Medium,
        }));
    }

    Ok(None)
}

fn extract_manifest_excerpt(scan: &ScanResult, service: &Service) -> Result<Option<String>> {
    let manifest_path = scan.repo_path.join(&service.path).join(&service.manifest);

    if !manifest_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

    let excerpt = if content.len() > 400 {
        format!("{}...", &content[..400])
    } else {
        content
    };

    Ok(Some(excerpt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let manifest = r#"{"main": "server.js", "scripts": {"start": "node server.js"}}"#;
        let prompt = build_prompt(&service, Some(manifest));

        assert!(prompt.contains("apps/web"));
        assert!(prompt.contains("npm"));
        assert!(prompt.contains("server.js"));
    }
}
