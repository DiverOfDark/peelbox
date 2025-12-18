use super::classify::{ClassifyResult, PackagePath, ServicePath};
use super::scan::ScanResult;
use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use crate::pipeline::Confidence;
use crate::stack::StackRegistry;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureResult {
    pub project_type: ProjectType,
    pub services: Vec<Service>,
    pub packages: Vec<Package>,
    pub orchestrator: Option<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Monorepo,
    SingleService,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub path: std::path::PathBuf,
    pub manifest: String,
    pub language: crate::stack::LanguageId,
    pub build_system: crate::stack::BuildSystemId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub path: std::path::PathBuf,
    pub manifest: String,
    pub language: crate::stack::LanguageId,
    pub build_system: crate::stack::BuildSystemId,
}

fn build_prompt(scan: &ScanResult, classify: &ClassifyResult) -> String {
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

    let config_files = discover_relevant_config_files(scan);

    format!(
        r#"Determine project structure and orchestrator.

Services: {}
Packages: {}
Config files: {}

Respond with JSON:
{{
  "project_type": "monorepo" | "singleservice",
  "orchestrator": "turborepo" | "nx" | "lerna" | "rush" | "bazel" | "pants" | "buck" | "none" | null,
  "confidence": "high" | "medium" | "low"
}}

Rules:
- "monorepo" if multiple services/packages exist
- "singleservice" if only one service at root
- orchestrator: Tool that coordinates builds across multiple packages (turbo.json=turborepo, nx.json=nx, lerna.json=lerna, rush.json=rush, etc.)
- Use "none" if no orchestrator detected
"#,
        serde_json::to_string(&services).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(&config_files).unwrap_or_else(|_| "[]".to_string())
    )
}

fn discover_relevant_config_files(scan: &ScanResult) -> Vec<String> {
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    let mut files_by_depth: HashMap<usize, Vec<&std::path::PathBuf>> = HashMap::new();

    for file in &scan.file_tree {
        let depth = file.components().count().saturating_sub(1);
        files_by_depth.entry(depth).or_default().push(file);
    }

    let mut result = Vec::new();
    let mut extension_counts: HashMap<String, Vec<String>> = HashMap::new();
    let max_files = 15;

    let mut depths: Vec<usize> = files_by_depth.keys().copied().collect();
    depths.sort();

    for depth in depths {
        if result.len() >= max_files {
            break;
        }

        if let Some(files) = files_by_depth.get(&depth) {
            for file in files {
                if result.len() >= max_files {
                    break;
                }

                let ext = file
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();

                let full_path = file.display().to_string();
                extension_counts
                    .entry(ext)
                    .or_default()
                    .push(full_path.clone());
                result.push(full_path);
            }
        }
    }

    let mut condensed = Vec::new();
    let mut shown_extensions: HashSet<String> = HashSet::new();

    for file_path in &result {
        let path = Path::new(file_path);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        if let Some(files_with_ext) = extension_counts.get(&ext) {
            if files_with_ext.len() == 1 {
                condensed.push(file_path.clone());
            } else if !shown_extensions.contains(&ext) {
                let ext_display = if ext.is_empty() {
                    "files without extension".to_string()
                } else {
                    format!(".{} files", ext)
                };
                condensed.push(format!(
                    "{}+{} more {}",
                    path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                    files_with_ext.len() - 1,
                    ext_display
                ));
                shown_extensions.insert(ext.clone());
            }
        }
    }

    condensed
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
        orchestrator: Option<String>,
        confidence: Confidence,
    }

    let llm_result: LLMStructure =
        super::llm_helper::query_llm_with_logging(llm_client, prompt, 500, "structure", logger)
            .await?;

    let services = build_services(scan, &classify.services);
    let packages = build_packages(scan, &classify.packages);

    let orchestrator = normalize_orchestrator(llm_result.orchestrator.as_deref())
        .or_else(|| detect_orchestrator_deterministic(scan));

    Ok(StructureResult {
        project_type: llm_result.project_type,
        services,
        packages,
        orchestrator,
        confidence: llm_result.confidence,
    })
}

fn can_use_deterministic(scan: &ScanResult, classify: &ClassifyResult) -> bool {
    let is_single =
        classify.services.len() == 1 && classify.packages.is_empty() && classify.root_is_service;

    let has_workspace = scan.workspace.has_workspace_config;

    is_single || has_workspace
}

fn deterministic_structure(scan: &ScanResult, classify: &ClassifyResult) -> StructureResult {
    let is_single =
        classify.services.len() == 1 && classify.packages.is_empty() && classify.root_is_service;

    if is_single {
        let services = build_services(scan, &classify.services);
        return StructureResult {
            project_type: ProjectType::SingleService,
            services,
            packages: vec![],
            orchestrator: None,
            confidence: Confidence::High,
        };
    }

    let services = build_services(scan, &classify.services);
    let packages = build_packages(scan, &classify.packages);
    let orchestrator = detect_orchestrator_deterministic(scan);

    StructureResult {
        project_type: ProjectType::Monorepo,
        services,
        packages,
        orchestrator,
        confidence: Confidence::High,
    }
}

fn normalize_orchestrator(llm_value: Option<&str>) -> Option<String> {
    match llm_value {
        Some("none") | None => None,
        Some(value) => {
            let normalized = value.to_lowercase();
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        }
    }
}

fn detect_orchestrator_deterministic(scan: &ScanResult) -> Option<String> {
    let registry = StackRegistry::with_defaults();
    let file_tree = &scan.file_tree;

    for orchestrator in registry.all_orchestrators() {
        for config_file in orchestrator.config_files() {
            if file_tree
                .iter()
                .any(|p| p.to_string_lossy().contains(config_file))
            {
                return Some(orchestrator.name().to_lowercase());
            }
        }
    }

    None
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

            let matched = scan.detections.iter().find(|d| {
                let detection_rel_path = d
                    .manifest_path
                    .strip_prefix(&scan.repo_path)
                    .unwrap_or(&d.manifest_path);

                let detection_filename = d
                    .manifest_path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or("");
                let sp_path_str = sp.path.to_str().unwrap_or(".");
                let detection_dir_raw = detection_rel_path
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("");
                // Normalize empty string to "." for comparison
                let detection_dir = if detection_dir_raw.is_empty() {
                    "."
                } else {
                    detection_dir_raw
                };

                tracing::debug!(
                    "Checking detection: manifest_path={}, relative={}, filename={}, dir={}",
                    d.manifest_path.display(),
                    detection_rel_path.display(),
                    detection_filename,
                    detection_dir
                );

                // Match if:
                // 1. Relative detection path matches the full service path (e.g., "app/Cargo.toml")
                // 2. Detection filename matches manifest AND directories match (e.g., "Cargo.toml" in "app" and "app")
                detection_rel_path == full_path
                    || (detection_filename == sp.manifest && detection_dir == sp_path_str)
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
                language: d.language,
                build_system: d.build_system,
            })
        })
        .collect()
}

fn build_packages(scan: &ScanResult, package_paths: &[PackagePath]) -> Vec<Package> {
    package_paths
        .iter()
        .filter_map(|pp| {
            let full_path = pp.path.join(&pp.manifest);
            scan.detections
                .iter()
                .find(|d| {
                    let detection_rel_path = d
                        .manifest_path
                        .strip_prefix(&scan.repo_path)
                        .unwrap_or(&d.manifest_path);

                    let detection_filename = d
                        .manifest_path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("");
                    let pp_path_str = pp.path.to_str().unwrap_or(".");
                    let detection_dir_raw = detection_rel_path
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("");
                    // Normalize empty string to "." for comparison
                    let detection_dir = if detection_dir_raw.is_empty() {
                        "."
                    } else {
                        detection_dir_raw
                    };

                    // Match if:
                    // 1. Relative detection path matches the full package path
                    // 2. Detection filename matches manifest AND directories match
                    detection_rel_path == full_path
                        || (detection_filename == pp.manifest && detection_dir == pp_path_str)
                })
                .map(|d| Package {
                    path: pp.path.clone(),
                    manifest: pp.manifest.clone(),
                    language: d.language,
                    build_system: d.build_system,
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {}
