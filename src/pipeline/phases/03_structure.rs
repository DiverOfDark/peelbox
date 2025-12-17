use super::classify::{ClassifyResult, PackagePath, ServicePath};
use crate::pipeline::Confidence;
use super::scan::ScanResult;
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureResult {
    pub project_type: ProjectType,
    pub monorepo_tool: Option<MonorepoTool>,
    pub services: Vec<Service>,
    pub packages: Vec<Package>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Monorepo,
    SingleService,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MonorepoTool {
    PnpmWorkspaces,
    YarnWorkspaces,
    NpmWorkspaces,
    Turborepo,
    Nx,
    Lerna,
    CargoWorkspace,
    GradleMultiproject,
    MavenMultimodule,
    GoWorkspace,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub path: std::path::PathBuf,
    pub manifest: String,
    pub language: String,
    pub build_system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub path: std::path::PathBuf,
    pub manifest: String,
    pub language: String,
    pub build_system: String,
}

fn build_prompt(_scan: &ScanResult, classify: &ClassifyResult) -> String {
    let services: Vec<String> = classify
        .services
        .iter()
        .map(|s| format!("{} ({})", s.path.display(), s.manifest))
        .collect();

    let packages: Vec<String> = classify
        .packages
        .iter()
        .map(|p| format!("{} ({})", p.path.display(), p.manifest))
        .collect();

    format!(
        r#"Determine project structure and monorepo tool (if applicable).

Services: {}
Packages: {}

Respond with JSON:
{{
  "project_type": "monorepo" | "singleservice",
  "monorepo_tool": "pnpmworkspaces" | "yarnworkspaces" | "npmworkspaces" | "turborepo" | "nx" | "lerna" | "cargoworkspace" | "gradlemultiproject" | "mavenmultimodule" | "goworkspace" | "unknown" | null,
  "confidence": "high" | "medium" | "low"
}}

Rules:
- "monorepo" if multiple services/packages exist
- "singleservice" if only one service at root
- Detect monorepo tool from workspace config files
"#,
        serde_json::to_string(&services).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string())
    )
}

pub async fn execute(
    llm_client: &dyn LLMClient,
    scan: &ScanResult,
    classify: &ClassifyResult,
    logger: &Arc<HeuristicLogger>,
) -> Result<StructureResult> {
    if can_use_deterministic(scan, classify) {
        return Ok(deterministic_structure(scan, classify));
    }

    let prompt = build_prompt(scan, classify);

    #[derive(Deserialize, Serialize)]
    struct LLMStructure {
        project_type: ProjectType,
        monorepo_tool: Option<MonorepoTool>,
        confidence: Confidence,
    }

    let llm_result: LLMStructure =
        super::llm_helper::query_llm_with_logging(llm_client, prompt, 500, "structure", logger)
            .await?;

    let services = build_services(scan, &classify.services);
    let packages = build_packages(scan, &classify.packages);

    Ok(StructureResult {
        project_type: llm_result.project_type,
        monorepo_tool: llm_result.monorepo_tool,
        services,
        packages,
        confidence: llm_result.confidence,
    })
}

fn can_use_deterministic(scan: &ScanResult, classify: &ClassifyResult) -> bool {
    let is_single =
        classify.services.len() == 1 && classify.packages.is_empty() && classify.root_is_service;

    let has_workspace = scan.bootstrap_context.workspace.has_workspace_config;

    is_single || has_workspace
}

fn deterministic_structure(scan: &ScanResult, classify: &ClassifyResult) -> StructureResult {
    let is_single =
        classify.services.len() == 1 && classify.packages.is_empty() && classify.root_is_service;

    if is_single {
        let services = build_services(scan, &classify.services);
        return StructureResult {
            project_type: ProjectType::SingleService,
            monorepo_tool: None,
            services,
            packages: vec![],
            confidence: Confidence::High,
        };
    }

    let monorepo_tool = detect_monorepo_tool(scan);
    let services = build_services(scan, &classify.services);
    let packages = build_packages(scan, &classify.packages);

    StructureResult {
        project_type: ProjectType::Monorepo,
        monorepo_tool: Some(monorepo_tool),
        services,
        packages,
        confidence: Confidence::High,
    }
}

fn detect_monorepo_tool(scan: &ScanResult) -> MonorepoTool {
    let file_tree = &scan.file_tree;

    if file_tree
        .iter()
        .any(|p| p.to_string_lossy().contains("pnpm-workspace.yaml"))
    {
        return MonorepoTool::PnpmWorkspaces;
    }

    if file_tree
        .iter()
        .any(|p| p.to_string_lossy().contains("turbo.json"))
    {
        return MonorepoTool::Turborepo;
    }

    if file_tree
        .iter()
        .any(|p| p.to_string_lossy().contains("nx.json"))
    {
        return MonorepoTool::Nx;
    }

    if file_tree
        .iter()
        .any(|p| p.to_string_lossy().contains("lerna.json"))
    {
        return MonorepoTool::Lerna;
    }

    for detection in &scan.bootstrap_context.detections {
        if detection.is_workspace_root {
            match detection.build_system.as_str() {
                "cargo" => return MonorepoTool::CargoWorkspace,
                "gradle" => return MonorepoTool::GradleMultiproject,
                "maven" => return MonorepoTool::MavenMultimodule,
                "go" => return MonorepoTool::GoWorkspace,
                "npm" => return MonorepoTool::NpmWorkspaces,
                "yarn" => return MonorepoTool::YarnWorkspaces,
                _ => {}
            }
        }
    }

    MonorepoTool::Unknown
}

fn build_services(scan: &ScanResult, service_paths: &[ServicePath]) -> Vec<Service> {
    service_paths
        .iter()
        .filter_map(|sp| {
            tracing::debug!(
                "Looking for service: path={}, manifest={}",
                sp.path.display(),
                sp.manifest
            );

            let full_path = sp.path.join(&sp.manifest);
            tracing::debug!("Full path: {}", full_path.display());

            let matched = scan.bootstrap_context.detections.iter().find(|d| {
                tracing::debug!("Checking detection: manifest_path={}", d.manifest_path);
                d.manifest_path == sp.manifest || d.manifest_path == full_path.to_string_lossy()
            });

            if matched.is_none() {
                tracing::warn!(
                    "No bootstrap detection found for service: {} (manifest: {})",
                    sp.path.display(),
                    sp.manifest
                );
            }

            matched.map(|d| Service {
                path: sp.path.clone(),
                manifest: sp.manifest.clone(),
                language: d.language.clone(),
                build_system: d.build_system.clone(),
            })
        })
        .collect()
}

fn build_packages(scan: &ScanResult, package_paths: &[PackagePath]) -> Vec<Package> {
    package_paths
        .iter()
        .filter_map(|pp| {
            scan.bootstrap_context
                .detections
                .iter()
                .find(|d| {
                    d.manifest_path == pp.manifest
                        || d.manifest_path == pp.path.join(&pp.manifest).to_string_lossy()
                })
                .map(|d| Package {
                    path: pp.path.clone(),
                    manifest: pp.manifest.clone(),
                    language: d.language.clone(),
                    build_system: d.build_system.clone(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_pnpm_workspaces() {
        let scan = create_scan_with_files(vec!["pnpm-workspace.yaml", "package.json"]);
        let tool = detect_monorepo_tool(&scan);
        assert_eq!(tool, MonorepoTool::PnpmWorkspaces);
    }

    #[test]
    fn test_detect_cargo_workspace() {
        let mut scan = create_scan_with_files(vec!["Cargo.toml"]);
        scan.bootstrap_context.detections[0].is_workspace_root = true;
        let tool = detect_monorepo_tool(&scan);
        assert_eq!(tool, MonorepoTool::CargoWorkspace);
    }

    fn create_scan_with_files(files: Vec<&str>) -> ScanResult {
        use crate::bootstrap::{BootstrapContext, LanguageDetection, RepoSummary, WorkspaceInfo};
        use std::collections::HashMap;
        use std::path::PathBuf;

        ScanResult {
            repo_path: PathBuf::from("."),
            bootstrap_context: BootstrapContext {
                summary: RepoSummary {
                    manifest_count: 1,
                    primary_language: Some("Rust".to_string()),
                    primary_build_system: Some("cargo".to_string()),
                    is_monorepo: false,
                    root_manifests: vec![],
                },
                detections: vec![LanguageDetection {
                    language: "Rust".to_string(),
                    build_system: "cargo".to_string(),
                    manifest_path: "Cargo.toml".to_string(),
                    depth: 0,
                    confidence: 1.0,
                    is_workspace_root: false,
                }],
                workspace: WorkspaceInfo {
                    root_manifests: vec![],
                    nested_by_depth: HashMap::new(),
                    max_depth: 0,
                    has_workspace_config: false,
                },
                scan_time_ms: 50,
            },
            file_tree: files.iter().map(|f| PathBuf::from(f)).collect(),
        }
    }
}
