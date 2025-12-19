use super::structure::Service;
use crate::pipeline::Confidence;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntrypointInfo {
    pub entrypoint: String,
    pub confidence: Confidence,
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
        service.build_system.name(),
        service.language.name(),
        manifest_excerpt.unwrap_or("None")
    )
}

use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use async_trait::async_trait;

pub struct EntrypointPhase;

#[async_trait]
impl ServicePhase for EntrypointPhase {
    fn name(&self) -> &'static str {
        "EntrypointPhase"
    }

    fn try_deterministic(&self, context: &mut ServiceContext) -> Result<Option<()>> {
        if let Some(deterministic) = try_deterministic_helper(context)? {
            context.entrypoint = Some(deterministic);
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    async fn execute_llm(&self, context: &mut ServiceContext) -> Result<()> {
        let manifest_excerpt = extract_manifest_excerpt(context)?;

        let prompt = build_prompt(context.service, manifest_excerpt.as_deref());
        let result = super::llm_helper::query_llm_with_logging(
            context.llm_client(),
            prompt,
            300,
            "entrypoint",
            context.heuristic_logger(),
        )
        .await?;

        context.entrypoint = Some(result);
        Ok(())
    }
}

fn try_deterministic_helper(context: &ServiceContext) -> Result<Option<EntrypointInfo>> {
    let language_def = match context
        .stack_registry()
        .get_language(context.service.language)
    {
        Some(def) => def,
        None => return Ok(None),
    };

    let manifest_path = context
        .scan()?
        .repo_path
        .join(&context.service.path)
        .join(&context.service.manifest);

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

    if let Some(entrypoint) = language_def.default_entrypoint(context.service.build_system.name()) {
        return Ok(Some(EntrypointInfo {
            entrypoint,
            confidence: Confidence::Medium,
        }));
    }

    Ok(None)
}

fn extract_manifest_excerpt(context: &ServiceContext) -> Result<Option<String>> {
    let manifest_path = context
        .scan()?
        .repo_path
        .join(&context.service.path)
        .join(&context.service.manifest);

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
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let manifest = r#"{"main": "server.js", "scripts": {"start": "node server.js"}}"#;
        let prompt = build_prompt(&service, Some(manifest));

        assert!(prompt.contains("apps/web"));
        assert!(prompt.contains("npm"));
        assert!(prompt.contains("server.js"));
    }
}
