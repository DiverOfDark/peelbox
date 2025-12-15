use super::scan::ScanResult;
use crate::llm::LLMClient;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

fn build_prompt(scan: &ScanResult) -> String {
    let manifest_list: Vec<String> = scan
        .bootstrap_context
        .detections
        .iter()
        .map(|d| format!("{} ({})", d.manifest_path, d.build_system))
        .collect();

    let is_monorepo = scan.bootstrap_context.summary.is_monorepo;

    format!(
        r#"Classify directories in this repository as either "service" (independently deployable application) or "package" (shared library/dependency).

Repository information:
- Is monorepo: {}
- Manifests found: {}

Classification rules:
- **Service**: Has a runnable entrypoint, can be deployed independently (e.g., web server, CLI tool, worker)
- **Package**: Shared library or utility code consumed by other services/packages
- **Root is service**: If root directory has a manifest and can be deployed as a standalone application

Respond with JSON:
{{
  "services": [
    {{"path": "apps/web", "manifest": "package.json"}},
    {{"path": ".", "manifest": "Cargo.toml"}}
  ],
  "packages": [
    {{"path": "packages/shared", "manifest": "package.json"}}
  ],
  "root_is_service": true,
  "confidence": "high"
}}

Note: Use "." for root directory. Confidence: "high" | "medium" | "low"
"#,
        is_monorepo,
        serde_json::to_string(&manifest_list).unwrap_or_else(|_| "[]".to_string())
    )
}

pub async fn execute(llm_client: &dyn LLMClient, scan: &ScanResult) -> Result<ClassifyResult> {
    if can_skip_llm(scan) {
        return Ok(deterministic_classify(scan));
    }

    let prompt = build_prompt(scan);

    let request = crate::llm::types::ChatRequest {
        messages: vec![crate::llm::types::Message {
            role: "user".to_string(),
            content: prompt,
        }],
        temperature: Some(0.1),
        max_tokens: Some(1000),
    };

    let response = llm_client
        .chat(request)
        .await
        .context("Failed to call LLM for classification")?;

    let result: ClassifyResult = serde_json::from_str(&response.content)
        .context("Failed to parse classification response")?;

    Ok(result)
}

fn can_skip_llm(scan: &ScanResult) -> bool {
    let detections = &scan.bootstrap_context.detections;

    if detections.len() == 1 && detections[0].depth == 0 {
        return true;
    }

    false
}

fn deterministic_classify(scan: &ScanResult) -> ClassifyResult {
    let detections = &scan.bootstrap_context.detections;

    if detections.len() == 1 && detections[0].depth == 0 {
        return ClassifyResult {
            services: vec![ServicePath {
                path: PathBuf::from("."),
                manifest: detections[0].manifest_path.clone(),
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
        use crate::bootstrap::{BootstrapContext, LanguageDetection, RepoSummary, WorkspaceInfo};
        use std::collections::HashMap;

        let detections = vec![LanguageDetection {
            language: "Rust".to_string(),
            build_system: "cargo".to_string(),
            manifest_path: "Cargo.toml".to_string(),
            depth: 0,
            confidence: 1.0,
            is_workspace_root: false,
        }];

        ScanResult {
            repo_path: PathBuf::from("."),
            bootstrap_context: BootstrapContext {
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
                scan_time_ms: 50,
            },
            file_tree: vec![PathBuf::from("Cargo.toml"), PathBuf::from("src/main.rs")],
        }
    }
}
