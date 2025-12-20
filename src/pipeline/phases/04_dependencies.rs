use super::service_analysis::Service;
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use crate::stack::language::{Dependency, DependencyInfo, DetectionMethod};
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

use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use async_trait::async_trait;

pub struct DependenciesPhase;

impl DependenciesPhase {
    fn process_item(
        scan: &super::scan::ScanResult,
        registry: &Arc<StackRegistry>,
        path: &PathBuf,
        manifest: &str,
        all_paths: &[PathBuf],
    ) -> Result<Option<DependencyInfo>> {
        let manifest_path = scan.repo_path.join(path).join(manifest);

        if !manifest_path.exists() {
            return Ok(None);
        }

        let manifest_content = std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

        let dep_info = registry.parse_dependencies_by_manifest(manifest, &manifest_content, all_paths);

        Ok(dep_info)
    }
}

#[async_trait]
impl WorkflowPhase for DependenciesPhase {
    fn name(&self) -> &'static str {
        "DependenciesPhase"
    }

    fn try_deterministic(&self, context: &mut AnalysisContext) -> Result<Option<()>> {
        let scan = context
            .scan
            .as_ref()
            .expect("Scan must be available before dependencies");
        let workspace = context
            .workspace
            .as_ref()
            .expect("Workspace must be available before dependencies");

        let registry = &context.stack_registry;
        let mut dependencies = HashMap::new();

        let all_paths: Vec<PathBuf> = workspace.packages.iter().map(|p| p.path.clone()).collect();

        // Match workspace packages with scan detections to get manifest info
        let all_items: Vec<_> = workspace
            .packages
            .iter()
            .filter_map(|package| {
                scan.detections
                    .iter()
                    .find(|detection| {
                        detection
                            .manifest_path
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new(""))
                            == package.path
                    })
                    .map(|detection| {
                        (
                            package.path.clone(),
                            detection
                                .manifest_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string(),
                        )
                    })
            })
            .collect();

        for (path, manifest) in all_items {
            if let Some(dep_info) = Self::process_item(scan, &registry, &path, &manifest, &all_paths)? {
                match dep_info {
                    info if info.detected_by == DetectionMethod::Deterministic => {
                        dependencies.insert(path, info);
                    }
                    _ => {
                        return Ok(None);
                    }
                }
            }
        }

        context.dependencies = Some(DependencyResult { dependencies });
        Ok(Some(()))
    }

    async fn execute_llm(&self, context: &mut AnalysisContext) -> Result<()> {
        let scan = context
            .scan
            .as_ref()
            .expect("Scan must be available before dependencies");
        let workspace = context
            .workspace
            .as_ref()
            .expect("Workspace must be available before dependencies");

        let registry = &context.stack_registry;
        let mut dependencies = HashMap::new();

        let all_paths: Vec<PathBuf> = workspace
            .packages
            .iter()
            .map(|p| p.path.clone())
            .collect();

        // Match workspace packages with scan detections to create Service structs
        let services: Vec<_> = workspace
            .packages
            .iter()
            .filter_map(|package| {
                scan.detections
                    .iter()
                    .find(|detection| {
                        detection
                            .manifest_path
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new(""))
                            == package.path
                    })
                    .map(|detection| Service {
                        path: package.path.clone(),
                        manifest: detection
                            .manifest_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        language: detection.language,
                        build_system: detection.build_system,
                    })
            })
            .collect();

        for service in &services {
            if let Some(dep_info) =
                Self::process_item(scan, &registry, &service.path, &service.manifest, &all_paths)?
            {
                let final_dep_info = match dep_info {
                    info if info.detected_by == DetectionMethod::Deterministic => info,
                    _ => {
                        let manifest_path = scan.repo_path.join(&service.path).join(&service.manifest);
                        let manifest_content = std::fs::read_to_string(&manifest_path)?;
                        llm_fallback(
                            context.llm_client.as_ref(),
                            service,
                            &manifest_content,
                            &all_paths,
                            &context.heuristic_logger,
                        )
                        .await?
                    }
                };
                dependencies.insert(service.path.clone(), final_dep_info);
            }
        }

        context.dependencies = Some(DependencyResult { dependencies });
        Ok(())
    }
}

