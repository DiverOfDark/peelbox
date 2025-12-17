use super::dependencies::DependencyResult;
use super::scan::ScanResult;
use super::structure::Service;
use crate::frameworks::FrameworkRegistry;
use crate::heuristics::HeuristicLogger;
use crate::languages::LanguageRegistry;
use crate::llm::LLMClient;
use crate::pipeline::Confidence;
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
    dependencies: &DependencyResult,
    framework_registry: &FrameworkRegistry,
    logger: &Arc<HeuristicLogger>,
) -> Result<RuntimeInfo> {
    if let Some(deterministic) = try_deterministic(service, dependencies, framework_registry) {
        return Ok(deterministic);
    }

    let files = extract_relevant_files(scan, service);
    let manifest_excerpt = extract_manifest_excerpt(scan, service)?;

    let prompt = build_prompt(service, &files, manifest_excerpt.as_deref());
    super::llm_helper::query_llm_with_logging(llm_client, prompt, 500, "runtime", logger).await
}

fn try_deterministic(
    service: &Service,
    dependencies: &DependencyResult,
    framework_registry: &FrameworkRegistry,
) -> Option<RuntimeInfo> {
    let registry = LanguageRegistry::with_defaults();
    let language_def = registry.get_language(&service.language)?;

    let runtime = language_def.runtime_name()?;

    let framework = dependencies
        .dependencies
        .get(&service.path)
        .and_then(|deps| framework_registry.detect_from_dependencies(deps))
        .map(|(fw, _confidence)| fw.name().to_string());

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;
    use std::collections::HashMap;

    #[test]
    fn test_deterministic_rust() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "Cargo.toml".to_string(),
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
        };

        let dependencies = DependencyResult {
            dependencies: HashMap::new(),
        };
        let framework_registry = FrameworkRegistry::new();

        let result = try_deterministic(&service, &dependencies, &framework_registry).unwrap();
        assert_eq!(result.runtime, "rust");
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.framework, None);
    }

    #[test]
    fn test_deterministic_node() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let dependencies = DependencyResult {
            dependencies: HashMap::new(),
        };
        let framework_registry = FrameworkRegistry::new();

        let result = try_deterministic(&service, &dependencies, &framework_registry).unwrap();
        assert_eq!(result.runtime, "node");
        assert_eq!(result.confidence, Confidence::High);
        assert_eq!(result.framework, None);
    }

    #[test]
    fn test_deterministic_with_framework() {
        let service = Service {
            path: PathBuf::from("."),
            manifest: "package.json".to_string(),
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
        };

        let mut deps_info = crate::languages::DependencyInfo::empty();
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
        let framework_registry = FrameworkRegistry::new();

        let result = try_deterministic(&service, &dependencies, &framework_registry).unwrap();
        assert_eq!(result.runtime, "node");
        assert_eq!(result.framework, Some("Express".to_string()));
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
