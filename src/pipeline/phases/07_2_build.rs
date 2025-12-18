use super::scan::ScanResult;
use super::structure::Service;
use crate::pipeline::Confidence;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    pub build_cmd: Option<String>,
    pub output_dir: Option<PathBuf>,
    pub confidence: Confidence,
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
        service.build_system.name(),
        service.language.name(),
        scripts_excerpt.unwrap_or("None")
    )
}

fn try_deterministic(service: &Service) -> Option<BuildInfo> {
    let build_system_registry = crate::stack::registry::StackRegistry::with_defaults();
    let build_system = build_system_registry.get_build_system(service.build_system)?;

    let template = build_system.build_template();

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

use crate::pipeline::phase_trait::{ServicePhase, ServicePhaseResult};
use crate::pipeline::service_context::ServiceContext;
use async_trait::async_trait;

pub struct BuildPhase;

#[async_trait]
impl ServicePhase for BuildPhase {
    async fn execute(&self, context: &ServiceContext<'_>) -> Result<ServicePhaseResult> {
        if let Some(deterministic) = try_deterministic(context.service) {
            return Ok(ServicePhaseResult::Build(deterministic));
        }

        let scripts_excerpt = extract_scripts_excerpt(context.scan(), context.service)?;

        let prompt = build_prompt(context.service, scripts_excerpt.as_deref());
        let result = super::llm_helper::query_llm_with_logging(
            context.llm_client(),
            prompt,
            400,
            "build",
            context.heuristic_logger(),
        )
        .await?;
        Ok(ServicePhaseResult::Build(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_cargo() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: crate::stack::LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Cargo,
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
            language: crate::stack::LanguageId::Java,
            build_system: crate::stack::BuildSystemId::Maven,
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
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let scripts = r#"{"build": "next build", "start": "next start"}"#;
        let prompt = build_prompt(&service, Some(scripts));

        assert!(prompt.contains("apps/web"));
        assert!(prompt.contains("npm"));
        assert!(prompt.contains("next build"));
    }
}
