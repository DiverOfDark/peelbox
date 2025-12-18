use super::scan::ScanResult;
use super::structure::{Service, StructureResult};
use crate::heuristics::HeuristicLogger;
use crate::languages::{Dependency, DependencyInfo, DetectionMethod};
use crate::llm::LLMClient;
use crate::stack::registry::StackRegistry;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyResult {
    pub dependencies: HashMap<PathBuf, DependencyInfo>,
}

fn build_llm_prompt(
    service: &Service,
    manifest_content: &str,
    all_service_paths: &[PathBuf],
) -> String {
    format!(
        r#"Extract internal dependencies from this manifest.

Service: {}
Build system: {}
Manifest content:
```
{}
```

All service/package paths in repository: {}

Respond with JSON:
{{
  "internal_deps": ["relative/path/to/dep1", "relative/path/to/dep2"],
  "external_deps": ["external-package-1", "external-package-2"]
}}

Rules:
- internal_deps: References to other services/packages in THIS repository
- external_deps: Third-party packages from registries (npm, crates.io, etc.)
- Return empty arrays if none found
"#,
        service.path.display(),
        service.build_system.name(),
        manifest_content,
        serde_json::to_string(&all_service_paths).unwrap_or_else(|_| "[]".to_string())
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    scan: &ScanResult,
    structure: &StructureResult,
    logger: &Arc<HeuristicLogger>,
) -> Result<DependencyResult> {
    let registry = Arc::new(StackRegistry::with_defaults());
    let mut dependencies = HashMap::new();

    let all_paths: Vec<PathBuf> = structure
        .services
        .iter()
        .map(|s| s.path.clone())
        .chain(structure.packages.iter().map(|p| p.path.clone()))
        .collect();

    for service in &structure.services {
        let manifest_path = scan.repo_path.join(&service.path).join(&service.manifest);

        if !manifest_path.exists() {
            continue;
        }

        let manifest_content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

        let dep_info = registry.parse_dependencies_by_manifest(
            &service.manifest,
            &manifest_content,
            &all_paths,
        );

        let final_dep_info = match dep_info {
            Some(info) if info.detected_by == DetectionMethod::Deterministic => info,
            _ => llm_fallback(llm_client, service, &manifest_content, &all_paths, logger).await?,
        };

        dependencies.insert(service.path.clone(), final_dep_info);
    }

    for package in &structure.packages {
        let manifest_path = scan.repo_path.join(&package.path).join(&package.manifest);

        if !manifest_path.exists() {
            continue;
        }

        let manifest_content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

        let dep_info = registry.parse_dependencies_by_manifest(
            &package.manifest,
            &manifest_content,
            &all_paths,
        );

        let final_dep_info = match dep_info {
            Some(info) if info.detected_by == DetectionMethod::Deterministic => info,
            _ => {
                let pseudo_service = Service {
                    path: package.path.clone(),
                    manifest: package.manifest.clone(),
                    language: package.language.clone(),
                    build_system: package.build_system.clone(),
                };
                llm_fallback(
                    llm_client,
                    &pseudo_service,
                    &manifest_content,
                    &all_paths,
                    logger,
                )
                .await?
            }
        };

        dependencies.insert(package.path.clone(), final_dep_info);
    }

    Ok(DependencyResult { dependencies })
}

async fn llm_fallback(
    llm_client: &dyn LLMClient,
    service: &Service,
    manifest_content: &str,
    all_paths: &[PathBuf],
    logger: &Arc<HeuristicLogger>,
) -> Result<DependencyInfo> {
    let prompt = build_llm_prompt(service, manifest_content, all_paths);

    #[derive(Deserialize, Serialize)]
    struct LLMDeps {
        internal_deps: Vec<String>,
        external_deps: Vec<String>,
    }

    let llm_deps: LLMDeps =
        super::llm_helper::query_llm_with_logging(llm_client, prompt, 800, "dependencies", logger)
            .await?;

    Ok(DependencyInfo {
        internal_deps: llm_deps
            .internal_deps
            .into_iter()
            .map(|name| Dependency {
                name,
                version: None,
                is_internal: true,
            })
            .collect(),
        external_deps: llm_deps
            .external_deps
            .into_iter()
            .map(|name| Dependency {
                name,
                version: None,
                is_internal: false,
            })
            .collect(),
        detected_by: DetectionMethod::LLM,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_llm_prompt() {
        let service = Service {
            path: PathBuf::from("apps/web"),
            manifest: "package.json".to_string(),
            language: crate::stack::LanguageId::JavaScript,
            build_system: crate::stack::BuildSystemId::Npm,
        };

        let manifest = r#"{"name": "web", "dependencies": {"@repo/shared": "workspace:*"}}"#;
        let paths = vec![PathBuf::from("apps/web"), PathBuf::from("packages/shared")];

        let prompt = build_llm_prompt(&service, manifest, &paths);

        assert!(prompt.contains("apps/web"));
        assert!(prompt.contains("npm"));
        assert!(prompt.contains("@repo/shared"));
    }
}
