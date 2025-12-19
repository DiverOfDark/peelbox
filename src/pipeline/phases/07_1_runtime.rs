use super::dependencies::DependencyResult;
use super::scan::ScanResult;
use super::structure::Service;
use crate::pipeline::Confidence;
use crate::stack::StackRegistry;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub runtime: String,
    pub runtime_version: Option<String>,
    pub framework: Option<String>,
    pub confidence: Confidence,
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
        service.build_system.name(),
        service.language.name(),
        file_list.join("\n"),
        manifest_excerpt.unwrap_or("None")
    )
}

fn try_deterministic(
    service: &Service,
    dependencies: &DependencyResult,
    stack_registry: &Arc<StackRegistry>,
) -> Option<RuntimeInfo> {
    let language_def = stack_registry.get_language(service.language)?;

    let runtime = language_def.runtime_name()?;

    let framework = dependencies
        .dependencies
        .get(&service.path)
        .and_then(|deps| {
            for fw_id in crate::stack::FrameworkId::all_variants() {
                if let Some(fw) = stack_registry.get_framework(*fw_id) {
                    let patterns = fw.dependency_patterns();
                    for pattern in &patterns {
                        if deps.external_deps.iter().any(|d| pattern.matches(d))
                            || deps.internal_deps.iter().any(|d| pattern.matches(d))
                        {
                            return Some(fw.id().name().to_string());
                        }
                    }
                }
            }
            None
        });

    Some(RuntimeInfo {
        runtime: runtime.to_string(),
        runtime_version: None,
        framework,
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

use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use async_trait::async_trait;

pub struct RuntimePhase;

#[async_trait]
impl ServicePhase for RuntimePhase {
    fn name(&self) -> &'static str {
        "RuntimePhase"
    }

    type Output = RuntimeInfo;

    async fn execute(&self, context: &ServiceContext) -> Result<RuntimeInfo> {
        if let Some(deterministic) = try_deterministic(
            context.service,
            context.dependencies()?,
            context.stack_registry(),
        ) {
            return Ok(deterministic);
        }

        let files = extract_relevant_files(context.scan()?, context.service);
        let manifest_excerpt = extract_manifest_excerpt(context.scan()?, context.service)?;

        let prompt = build_prompt(context.service, &files, manifest_excerpt.as_deref());
        let result = super::llm_helper::query_llm_with_logging(
            context.llm_client(),
            prompt,
            500,
            "runtime",
            context.heuristic_logger(),
        )
        .await?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;
    use std::collections::HashMap;

    #[test]
    fn test_deterministic_rust() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: crate::stack::LanguageId::Rust,
            build_system: crate::stack::BuildSystemId::Cargo,
        };

        let dependencies = DependencyResult {
            dependencies: HashMap::new(),
        };
        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());

        let result = try_deterministic(&service, &dependencies, &stack_registry).unwrap();
        assert_eq!(result.runtime, "rust");
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.framework, None);
    }

    #[test]
    fn test_deterministic_node() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let dependencies = DependencyResult {
            dependencies: HashMap::new(),
        };
        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());

        let result = try_deterministic(&service, &dependencies, &stack_registry).unwrap();
        assert_eq!(result.runtime, "node");
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.framework, None);
    }

    #[test]
    fn test_deterministic_with_framework() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let mut deps_info = crate::stack::language::DependencyInfo::empty();
        deps_info.external_deps.push(Dependency {
            name: "express".to_string(),
            version: Some("4.18.0".to_string()),
            is_internal: false,
        });

        let mut deps_map = HashMap::new();
        deps_map.insert(PathBuf::from("."), deps_info);

        let dependencies = DependencyResult {
            dependencies: deps_map,
        };
        let stack_registry = Arc::new(crate::stack::StackRegistry::with_defaults());

        let result = try_deterministic(&service, &dependencies, &stack_registry).unwrap();
        assert_eq!(result.runtime, "node");
        assert_eq!(result.framework, Some("Express".to_string()));
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_build_prompt() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
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
