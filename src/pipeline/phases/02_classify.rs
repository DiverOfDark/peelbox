use super::scan::ScanResult;
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use crate::pipeline::Confidence;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyResult {
    pub services: Vec<ServicePath>,
    pub packages: Vec<PackagePath>,
    pub root_is_service: bool,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePath {
    pub path: PathBuf,
    pub manifest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagePath {
    pub path: PathBuf,
    pub manifest: String,
}

fn build_prompt(scan: &ScanResult) -> String {
    let manifest_list: Vec<String> = scan
        .detections
        .iter()
        .map(|d| {
            let dir = d
                .manifest_path
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or(".");
            let file = d
                .manifest_path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("");
            format!(
                "- {} in directory '{}' ({})",
                file,
                dir,
                d.build_system.name()
            )
        })
        .collect();

    let is_monorepo = scan.summary.is_monorepo;

    format!(
        r#"Classify directories in this repository as either "service" (independently deployable application) or "package" (shared library/dependency).

Repository information:
- Is monorepo: {}
- Manifests detected:
{}

Classification rules:
- **Service**: Has a runnable entrypoint, can be deployed independently (e.g., web server, CLI tool, worker)
- **Package**: Shared library or utility code consumed by other services/packages
- **Root is service**: If root directory has a manifest and can be deployed as a standalone application

You MUST only reference manifests from the list above. Do NOT invent or hallucinate manifests that were not detected.

Respond with JSON containing ONLY the detected manifests:
{{
  "services": [
    {{"path": "directory/path", "manifest": "manifest-filename.ext"}}
  ],
  "packages": [
    {{"path": "directory/path", "manifest": "manifest-filename.ext"}}
  ],
  "root_is_service": true,
  "confidence": "high"
}}

IMPORTANT:
- Use "." for root directory
- Use ONLY the manifest filenames from the detected list above
- Do NOT include manifests like package.json, Cargo.toml, etc. unless they appear in the detected list
- Confidence: "high" | "medium" | "low"
"#,
        is_monorepo,
        manifest_list.join("\n")
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    scan: &ScanResult,
    logger: &Arc<HeuristicLogger>,
) -> Result<ClassifyResult> {
    if can_skip_llm(scan) {
        return Ok(deterministic_classify(scan));
    }

    let prompt = build_prompt(scan);
    super::llm_helper::query_llm_with_logging(llm_client, prompt, 1000, "classify", logger).await
}

fn can_skip_llm(scan: &ScanResult) -> bool {
    let detections = &scan.detections;

    if detections.len() == 1 && detections[0].depth == 0 {
        return true;
    }

    false
}

fn deterministic_classify(scan: &ScanResult) -> ClassifyResult {
    let detections = &scan.detections;

    if detections.len() == 1 && detections[0].depth == 0 {
        return ClassifyResult {
            services: vec![ServicePath {
                path: PathBuf::from("."),
                manifest: detections[0].manifest_path.to_string_lossy().to_string(),
            }],
            packages: vec![],
            root_is_service: true,
            confidence: Confidence::High,
        };
    }

    ClassifyResult {
        services: vec![],
        packages: vec![],
        root_is_service: false,
        confidence: Confidence::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_classify_single_service() {
        let scan = create_single_service_scan();
        let result = deterministic_classify(&scan);

        assert_eq!(result.services.len(), 1);
        assert_eq!(result.packages.len(), 0);
        assert!(result.root_is_service);
        assert_eq!(result.confidence, Confidence::High);
    }

    fn create_single_service_scan() -> ScanResult {
        use crate::pipeline::phases::scan::{RepoSummary, WorkspaceInfo};
        use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
        use std::collections::HashMap;

        let detections = vec![DetectionStack::new(
            BuildSystemId::Cargo,
            LanguageId::Rust,
            PathBuf::from("Cargo.toml"),
        )
        .with_depth(0)
        .with_confidence(1.0)
        .with_workspace_root(false)];

        ScanResult {
            repo_path: PathBuf::from("."),
            summary: RepoSummary {
                manifest_count: 1,
                primary_language: Some("Rust".to_string()),
                primary_build_system: Some("cargo".to_string()),
                is_monorepo: false,
                root_manifests: vec!["Cargo.toml".to_string()],
            },
            detections,
            workspace: WorkspaceInfo {
                root_manifests: vec!["Cargo.toml".to_string()],
                nested_by_depth: HashMap::new(),
                max_depth: 0,
                has_workspace_config: false,
            },
            file_tree: vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/main.rs")],
            scan_time_ms: 50,
        }
    }
}
