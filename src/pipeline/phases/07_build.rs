use super::scan::ScanResult;
use super::structure::Service;
use crate::heuristics::HeuristicLogger;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

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
    logger: &Arc<HeuristicLogger>,
) -> Result<BuildInfo> {
    if let Some(deterministic) = try_deterministic(service) {
        return Ok(deterministic);
    }

    let scripts_excerpt = extract_scripts_excerpt(scan, service)?;

    let prompt = build_prompt(service, scripts_excerpt.as_deref());
    super::llm_helper::query_llm_with_logging(llm_client, prompt, 400, "build", logger).await
}

fn try_deterministic(service: &Service) -> Option<BuildInfo> {
    let registry = LanguageRegistry::with_defaults();
    let language_def = registry.get_language(&service.language)?;

    let template = language_def.build_template(&service.build_system)?;

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
                let excerpt =
                    serde_json::to_string_pretty(scripts).unwrap_or_else(|_| "{}".to_string());
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
        assert_eq!(
            result.build_cmd,
            Some("mvn clean package -DskipTests".to_string())
        );
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
