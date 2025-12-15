use super::scan::ScanResult;
use super::structure::Service;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    pub build_cmd: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

fn build_prompt(service: &Service, scripts_excerpt: Option<&str>) -> String {
    format!(
        r#"Detect the build command and output directory for this service.

Service path: {}
Build system: {}
Language: {}

Scripts/config excerpt:
{}

Respond with JSON:
{{
  "build_cmd": "npm run build" | "cargo build --release" | null,
  "output_dir": "dist" | "target/release" | "build" | null,
  "confidence": "high" | "medium" | "low"
}}

Rules:
- build_cmd: Command to build artifacts (null if no build step needed)
- output_dir: Where build artifacts are placed (relative to service root)
- For interpreted languages without compilation, both may be null
"#,
        service.path.display(),
        service.build_system,
        service.language,
        scripts_excerpt.unwrap_or("None")
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    service: &Service,
    scan: &ScanResult,
) -> Result<BuildInfo> {
    if let Some(deterministic) = try_deterministic(service) {
        return Ok(deterministic);
    }

    let scripts_excerpt = extract_scripts_excerpt(scan, service)?;

    let prompt = build_prompt(service, scripts_excerpt.as_deref());

    let request = crate::llm::types::ChatRequest {
        messages: vec![crate::llm::types::Message {
            role: "user".to_string(),
            content: prompt,
        }],
        temperature: Some(0.1),
        max_tokens: Some(400),
    };

    let response = llm_client
        .chat(request)
        .await
        .context("Failed to call LLM for build detection")?;

    let build_info: BuildInfo = serde_json::from_str(&response.content)
        .context("Failed to parse build detection response")?;

    Ok(build_info)
}

fn try_deterministic(service: &Service) -> Option<BuildInfo> {
    let registry = LanguageRegistry::new();
    let language_def = registry.get_by_name(&service.language)?;

    let template = language_def.build_template(&service.build_system)?;

    let build_cmd = template.build_commands.first().cloned();
    let output_dir = template.artifacts.first().and_then(|artifact| {
        let path_str = artifact.replace("{project_name}", "");
        let path = PathBuf::from(path_str);
        path.parent().map(|p| p.to_path_buf())
    });

    Some(BuildInfo {
        build_cmd,
        output_dir,
        confidence: Confidence::High,
    })
}

fn extract_scripts_excerpt(scan: &ScanResult, service: &Service) -> Result<Option<String>> {
    let manifest_path = scan.repo_path.join(&service.path).join(&service.manifest);

    if !manifest_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

    if service.manifest == "package.json" {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(scripts) = json.get("scripts") {
                let excerpt = serde_json::to_string_pretty(scripts)
                    .unwrap_or_else(|_| "{}".to_string());
                return Ok(Some(excerpt));
            }
        }
    }

    let excerpt = if content.len() > 300 {
        format!("{}...", &content[..300])
    } else {
        content
    };

    Ok(Some(excerpt))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_cargo() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
        };

        let result = try_deterministic(&service).unwrap();
        assert_eq!(result.build_cmd, Some("cargo build --release".to_string()));
        assert_eq!(result.output_dir, Some(PathBuf::from("target/release")));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_deterministic_maven() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "pom.xml".to_string(),
            language: "Java".to_string(),
            build_system: "maven".to_string(),
        };

        let result = try_deterministic(&service).unwrap();
        assert_eq!(result.build_cmd, Some("mvn package".to_string()));
        assert_eq!(result.output_dir, Some(PathBuf::from("target")));
    }

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let scripts = r#"{"build": "next build", "start": "next start"}"#;
        let prompt = build_prompt(&service, Some(scripts));

        assert!(prompt.contains("apps/web"));
        assert!(prompt.contains("npm"));
        assert!(prompt.contains("next build"));
    }
}
