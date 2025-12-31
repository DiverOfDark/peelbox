use crate::pipeline::context::AnalysisContext;
use crate::pipeline::phase_trait::WorkflowPhase;
use crate::stack::{DetectionStack, StackRegistry};
use anyhow::{Context, Result};
use async_trait::async_trait;
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, trace, warn};

#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub max_depth: usize,
    pub max_files: usize,
    pub read_content: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_depth: 10,
            max_files: 1000,
            read_content: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub manifest_count: usize,
    pub primary_language: Option<String>,
    pub primary_build_system: Option<String>,
    pub is_monorepo: bool,
    pub root_manifests: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub root_manifests: Vec<String>,
    pub nested_by_depth: BTreeMap<usize, Vec<String>>,
    pub max_depth: usize,
    pub has_workspace_config: bool,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub repo_path: PathBuf,
    pub summary: RepoSummary,
    pub detections: Vec<DetectionStack>,
    pub workspace: WorkspaceInfo,
    pub file_tree: Vec<PathBuf>,
    pub scan_time_ms: u64,
}

impl ScanResult {
    pub fn get_files_in_dir(&self, dir: &Path) -> Vec<PathBuf> {
        self.file_tree
            .iter()
            .filter(|p| p.starts_with(dir))
            .cloned()
            .collect()
    }

    pub fn find_files_by_name(&self, filename: &str) -> Vec<PathBuf> {
        self.file_tree
            .iter()
            .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some(filename))
            .cloned()
            .collect()
    }

    pub fn format_for_prompt(&self) -> String {
        let manifest_list: Vec<String> = self
            .detections
            .iter()
            .map(|d| d.manifest_path.to_string_lossy().to_string())
            .collect();

        let languages: Vec<String> = self
            .detections
            .iter()
            .filter(|d| d.depth == 0)
            .map(|d| format!("{} ({})", d.language.name(), d.build_system.name()))
            .collect();

        let workspace_roots: Vec<String> = self
            .detections
            .iter()
            .filter(|d| d.is_workspace_root)
            .map(|d| d.manifest_path.to_string_lossy().to_string())
            .collect();

        let workspace_info = if !workspace_roots.is_empty() {
            format!(
                "\n- Workspace Roots: {} (indicates monorepo with multiple sub-projects)",
                workspace_roots.join(", ")
            )
        } else {
            String::new()
        };

        format!(
            r#"## Pre-scanned Repository Analysis

**Manifests Found:** {} files
**Files:** {}

**Detected Languages:**
{}

**Project Structure:**
- Primary Language: {}
- Primary Build System: {}
- Is Monorepo: {}
- Root Manifests: {}{}

Use these manifest files to guide your analysis. Read them directly without searching."#,
            manifest_list.len(),
            serde_json::to_string(&manifest_list).unwrap_or_else(|_| "[]".to_string()),
            if languages.is_empty() {
                "- None detected".to_string()
            } else {
                languages
                    .iter()
                    .map(|l| format!("- {}", l))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            self.summary
                .primary_language
                .as_ref()
                .unwrap_or(&"unknown".to_string()),
            self.summary
                .primary_build_system
                .as_ref()
                .unwrap_or(&"unknown".to_string()),
            self.summary.is_monorepo,
            self.summary.root_manifests.join(", "),
            workspace_info
        )
    }

    fn from_scan(
        repo_path: PathBuf,
        detections: Vec<DetectionStack>,
        file_tree: Vec<PathBuf>,
        has_workspace_config: bool,
        scan_time_ms: u64,
    ) -> Self {
        let workspace = Self::build_workspace_info(&detections, has_workspace_config);
        let summary = Self::build_summary(&detections, &workspace);

        Self {
            repo_path,
            summary,
            detections,
            workspace,
            file_tree,
            scan_time_ms,
        }
    }

    fn build_workspace_info(
        detections: &[DetectionStack],
        has_workspace_config: bool,
    ) -> WorkspaceInfo {
        let mut root_manifests = Vec::new();
        let mut nested_by_depth: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        let mut max_depth = 0;

        for detection in detections {
            let manifest_path = detection.manifest_path.to_string_lossy().to_string();
            if detection.depth == 0 {
                root_manifests.push(manifest_path);
            } else {
                nested_by_depth
                    .entry(detection.depth)
                    .or_default()
                    .push(manifest_path);
                max_depth = max_depth.max(detection.depth);
            }
        }

        // Sort for deterministic serialization
        root_manifests.sort();
        for paths in nested_by_depth.values_mut() {
            paths.sort();
        }

        WorkspaceInfo {
            root_manifests,
            nested_by_depth,
            max_depth,
            has_workspace_config,
        }
    }

    fn build_summary(detections: &[DetectionStack], workspace: &WorkspaceInfo) -> RepoSummary {
        let primary = detections
            .iter()
            .filter(|d| d.depth == 0)
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap());

        let has_workspace_root = detections.iter().any(|d| d.is_workspace_root);

        let is_monorepo = workspace.has_workspace_config
            || has_workspace_root
            || workspace.max_depth > 1
            || detections.iter().filter(|d| d.depth > 0).count() > 2;

        RepoSummary {
            manifest_count: detections.len(),
            primary_language: primary.map(|d| d.language.name().to_string()),
            primary_build_system: primary.map(|d| d.build_system.name().to_string()),
            is_monorepo,
            root_manifests: workspace.root_manifests.clone(),
        }
    }
}

fn deduplicate_detections(
    detections: Vec<DetectionStack>,
    stack_registry: &Arc<StackRegistry>,
) -> Vec<DetectionStack> {
    // Group by directory - keep highest priority in each directory
    let mut by_directory: HashMap<PathBuf, Vec<DetectionStack>> = HashMap::new();

    for detection in detections {
        let dir = detection
            .manifest_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        by_directory.entry(dir).or_default().push(detection);
    }

    let mut deduplicated = Vec::new();

    for (_dir, mut detections_for_dir) in by_directory {
        if detections_for_dir.len() == 1 {
            deduplicated.extend(detections_for_dir);
        } else {
            // Multiple manifests in same directory - choose highest priority
            detections_for_dir.sort_by_key(|d| {
                let build_system = stack_registry.get_build_system(d.build_system.clone());
                let priority = build_system
                    .and_then(|bs| {
                        bs.manifest_patterns()
                            .iter()
                            .find(|p| {
                                d.manifest_path.file_name()
                                    == Some(std::ffi::OsStr::new(&p.filename))
                            })
                            .map(|p| p.priority)
                    })
                    .unwrap_or(0);
                std::cmp::Reverse(priority)
            });

            if let Some(highest_priority) = detections_for_dir.first() {
                deduplicated.push(highest_priority.clone());
            }
        }
    }

    // Sort by manifest path for deterministic ordering
    deduplicated.sort_by(|a, b| a.manifest_path.cmp(&b.manifest_path));

    deduplicated
}

fn enrich_detections(
    detections: &mut [DetectionStack],
    repo_path: &Path,
    stack_registry: &Arc<StackRegistry>,
    read_content: bool,
) -> Result<()> {
    for detection in detections.iter_mut() {
        let rel_path = detection
            .manifest_path
            .strip_prefix(repo_path)
            .unwrap_or(&detection.manifest_path);

        detection.depth = rel_path.to_string_lossy().matches('/').count();

        if read_content {
            if let Some(filename) = detection.manifest_path.file_name().and_then(|n| n.to_str()) {
                let content = std::fs::read_to_string(&detection.manifest_path).ok();
                detection.is_workspace_root =
                    stack_registry.is_workspace_root(filename, content.as_deref());
            }
        }
    }
    Ok(())
}

pub struct ScanPhase;

#[async_trait]
impl WorkflowPhase for ScanPhase {
    fn name(&self) -> &'static str {
        "ScanPhase"
    }

    async fn execute(&self, context: &mut AnalysisContext) -> Result<()> {
        self.scan_repository(context)
    }
}

impl ScanPhase {
    fn scan_repository(&self, context: &mut AnalysisContext) -> Result<()> {
        let config = ScanConfig::default();
        let repo_path = &context.repo_path;

        if !repo_path.exists() {
            return Err(anyhow::anyhow!(
                "Repository path does not exist: {:?}",
                repo_path
            ));
        }
        if !repo_path.is_dir() {
            return Err(anyhow::anyhow!(
                "Repository path is not a directory: {:?}",
                repo_path
            ));
        }

        let repo_path = repo_path
            .canonicalize()
            .context("Failed to canonicalize repository path")?;

        let stack_registry = Arc::clone(&context.stack_registry);

        debug!(
            repo_path = %repo_path.display(),
            "Starting repository scan"
        );

        let start = Instant::now();

        info!(
            repo = %repo_path.display(),
            max_depth = config.max_depth,
            max_files = config.max_files,
            "Starting repository scan"
        );

        let mut file_tree = Vec::new();
        let mut files_scanned = 0;
        let mut has_workspace_config = false;

        let mut override_builder = OverrideBuilder::new(&repo_path);
        for excluded in stack_registry.all_excluded_dirs() {
            override_builder.add(&format!("!{}/", excluded)).ok();
        }
        let overrides = override_builder
            .build()
            .unwrap_or_else(|_| OverrideBuilder::new(&repo_path).build().unwrap());

        let has_git_dir = repo_path.join(".git").exists();

        for result in WalkBuilder::new(&repo_path)
            .max_depth(Some(config.max_depth))
            .hidden(false)
            .git_ignore(has_git_dir)
            .git_global(false)
            .git_exclude(false)
            .overrides(overrides)
            .build()
        {
            let entry = match result {
                Ok(e) => e,
                Err(err) => {
                    warn!(error = %err, "Failed to read directory entry");
                    continue;
                }
            };
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if files_scanned >= config.max_files {
                warn!(
                    files_scanned,
                    max_files = config.max_files,
                    "Reached file limit, stopping scan"
                );
                break;
            }
            files_scanned += 1;

            let rel_path = path.strip_prefix(&repo_path).unwrap_or(path).to_path_buf();

            file_tree.push(rel_path.clone());

            trace!(
                path = %path.display(),
                "Added file to tree"
            );
        }

        info!(
            files_scanned,
            "File tree scan complete, running batch detection"
        );

        let fs = crate::fs::RealFileSystem;
        let mut detections = stack_registry.detect_all_stacks(&repo_path, &file_tree, &fs)?;

        // Register LLM languages and build systems for any Custom IDs detected
        for detection in &detections {
            if matches!(detection.language, crate::stack::LanguageId::Custom(_)) {
                stack_registry.register_llm_language(detection.language.clone());
            }
            if matches!(
                detection.build_system,
                crate::stack::BuildSystemId::Custom(_)
            ) {
                let manifest_path = repo_path.join(&detection.manifest_path);
                if let Err(e) = stack_registry.register_llm_build_system(
                    detection.build_system.clone(),
                    &manifest_path,
                    &fs,
                ) {
                    warn!(
                        "Failed to register LLM build system {:?}: {}",
                        detection.build_system, e
                    );
                }
            }
        }

        enrich_detections(
            &mut detections,
            &repo_path,
            &stack_registry,
            config.read_content,
        )?;

        for detection in &detections {
            debug!(
                path = %detection.manifest_path.display(),
                language = %detection.language.name(),
                build_system = %detection.build_system.name(),
                confidence = detection.confidence,
                "Detected language"
            );

            if detection.is_workspace_root {
                has_workspace_config = true;
            }
        }

        let elapsed = start.elapsed();
        let scan_time_ms = elapsed.as_millis() as u64;

        let detections = deduplicate_detections(detections, &stack_registry);

        info!(
            detections_found = detections.len(),
            files_scanned, scan_time_ms, "Repository scan completed"
        );

        let result = ScanResult::from_scan(
            repo_path,
            detections,
            file_tree,
            has_workspace_config,
            scan_time_ms,
        );

        context.scan = Some(result);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heuristics::HeuristicLogger;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        fs::create_dir(base.join(".git")).unwrap();

        fs::File::create(base.join("Cargo.toml"))
            .unwrap()
            .write_all(b"[package]\nname = \"test\"\nversion = \"0.1.0\"")
            .unwrap();

        fs::File::create(base.join("package.json"))
            .unwrap()
            .write_all(b"{\"name\": \"test\", \"version\": \"1.0.0\"}")
            .unwrap();

        fs::create_dir_all(base.join("crates/lib")).unwrap();
        fs::File::create(base.join("crates/lib/Cargo.toml"))
            .unwrap()
            .write_all(b"[package]\nname = \"lib\"")
            .unwrap();

        fs::create_dir(base.join("node_modules")).unwrap();
        fs::File::create(base.join("node_modules/package.json"))
            .unwrap()
            .write_all(b"{\"name\": \"ignored\"}")
            .unwrap();

        dir
    }

    fn create_test_context(repo_path: &Path) -> AnalysisContext {
        use crate::config::DetectionMode;
        let stack_registry = Arc::new(StackRegistry::with_defaults(None));
        let wolfi_index = Arc::new(crate::validation::WolfiPackageIndex::for_tests());
        let heuristic_logger = Arc::new(HeuristicLogger::disabled());
        AnalysisContext::new(
            repo_path,
            stack_registry,
            wolfi_index,
            None,
            heuristic_logger,
            DetectionMode::Full,
        )
    }

    #[tokio::test]
    async fn test_scan_execution() {
        let temp_dir = create_test_repo();
        let mut context = create_test_context(temp_dir.path());
        let phase = ScanPhase;
        phase.execute(&mut context).await.unwrap();
        assert!(context.scan.is_some());
    }

    #[tokio::test]
    async fn test_file_tree_excludes_node_modules() {
        let temp_dir = create_test_repo();
        let mut context = create_test_context(temp_dir.path());
        let phase = ScanPhase;
        phase.execute(&mut context).await.unwrap();

        let scan = context.scan.as_ref().unwrap();
        let paths: Vec<String> = scan
            .file_tree
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        assert!(!paths.iter().any(|p| p.contains("node_modules")));
    }

    #[tokio::test]
    async fn test_find_files_by_name() {
        let temp_dir = create_test_repo();
        let mut context = create_test_context(temp_dir.path());
        let phase = ScanPhase;
        phase.execute(&mut context).await.unwrap();

        let scan = context.scan.as_ref().unwrap();
        let cargo_files = scan.find_files_by_name("Cargo.toml");
        assert!(!cargo_files.is_empty());
    }
}
