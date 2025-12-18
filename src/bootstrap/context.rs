use crate::stack::DetectionStack;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapContext {
    pub summary: RepoSummary,
    pub detections: Vec<DetectionStack>,
    pub workspace: WorkspaceInfo,
    pub scan_time_ms: u64,
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
    pub nested_by_depth: HashMap<usize, Vec<String>>,
    pub max_depth: usize,
    pub has_workspace_config: bool,
}

impl BootstrapContext {
    pub fn from_detections(
        detections: Vec<DetectionStack>,
        has_workspace_config: bool,
        scan_time_ms: u64,
    ) -> Self {
        let workspace = Self::build_workspace_info(&detections, has_workspace_config);
        let summary = Self::build_summary(&detections, &workspace);

        Self {
            summary,
            detections,
            workspace,
            scan_time_ms,
        }
    }

    fn build_workspace_info(
        detections: &[DetectionStack],
        has_workspace_config: bool,
    ) -> WorkspaceInfo {
        let mut root_manifests = Vec::new();
        let mut nested_by_depth: HashMap<usize, Vec<String>> = HashMap::new();
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_detections() -> Vec<DetectionStack> {
        use crate::stack::{BuildSystemId, LanguageId};
        use std::path::PathBuf;

        vec![
            DetectionStack::new(
                BuildSystemId::Cargo,
                LanguageId::Rust,
                PathBuf::from("Cargo.toml"),
            )
            .with_depth(0)
            .with_confidence(1.0)
            .with_workspace_root(true),
            DetectionStack::new(
                BuildSystemId::Npm,
                LanguageId::JavaScript,
                PathBuf::from("package.json"),
            )
            .with_depth(0)
            .with_confidence(0.8)
            .with_workspace_root(false),
            DetectionStack::new(
                BuildSystemId::Cargo,
                LanguageId::Rust,
                PathBuf::from("crates/lib/Cargo.toml"),
            )
            .with_depth(2)
            .with_confidence(1.0)
            .with_workspace_root(false),
        ]
    }

    #[test]
    fn test_context_creation() {
        let detections = create_test_detections();
        let context = BootstrapContext::from_detections(detections, false, 50);

        assert_eq!(context.detections.len(), 3);
        assert_eq!(context.scan_time_ms, 50);
    }

    #[test]
    fn test_summary_primary_detection() {
        let detections = create_test_detections();
        let context = BootstrapContext::from_detections(detections, false, 50);

        assert_eq!(context.summary.primary_language, Some("Rust".to_string()));
        assert_eq!(
            context.summary.primary_build_system,
            Some("Cargo".to_string())
        );
    }

    #[test]
    fn test_monorepo_detection_by_depth() {
        let detections = create_test_detections();
        let context = BootstrapContext::from_detections(detections, false, 50);

        assert!(context.summary.is_monorepo);
    }

    #[test]
    fn test_monorepo_detection_by_config() {
        use crate::stack::{BuildSystemId, LanguageId};
        use std::path::PathBuf;

        let detections = vec![DetectionStack::new(
            BuildSystemId::Npm,
            LanguageId::JavaScript,
            PathBuf::from("package.json"),
        )
        .with_depth(0)
        .with_confidence(0.9)
        .with_workspace_root(false)];
        let context = BootstrapContext::from_detections(detections, true, 50);

        assert!(context.summary.is_monorepo);
    }

    #[test]
    fn test_workspace_info() {
        let detections = create_test_detections();
        let context = BootstrapContext::from_detections(detections, false, 50);

        assert_eq!(context.workspace.root_manifests.len(), 2);
        assert!(context
            .workspace
            .root_manifests
            .contains(&"Cargo.toml".to_string()));
        assert_eq!(context.workspace.max_depth, 2);
    }

    #[test]
    fn test_format_for_prompt() {
        let detections = create_test_detections();
        let context = BootstrapContext::from_detections(detections, false, 50);

        let prompt = context.format_for_prompt();

        assert!(prompt.contains("Pre-scanned Repository"));
        assert!(prompt.contains("Cargo.toml"));
        assert!(prompt.contains("package.json"));
        assert!(prompt.contains("Rust"));
        assert!(prompt.contains("Cargo"));
    }

    #[test]
    fn test_empty_detections() {
        let context = BootstrapContext::from_detections(vec![], false, 10);

        assert_eq!(context.summary.manifest_count, 0);
        assert!(context.summary.primary_language.is_none());
        assert!(!context.summary.is_monorepo);
    }

    #[test]
    fn test_monorepo_detection_by_workspace_root() {
        use crate::stack::{BuildSystemId, LanguageId};
        use std::path::PathBuf;

        let detections = vec![DetectionStack::new(
            BuildSystemId::Cargo,
            LanguageId::Rust,
            PathBuf::from("Cargo.toml"),
        )
        .with_depth(0)
        .with_confidence(1.0)
        .with_workspace_root(true)];
        let context = BootstrapContext::from_detections(detections, false, 50);

        assert!(context.summary.is_monorepo);
    }

    #[test]
    fn test_format_for_prompt_includes_workspace_roots() {
        use crate::stack::{BuildSystemId, LanguageId};
        use std::path::PathBuf;

        let detections = vec![
            DetectionStack::new(
                BuildSystemId::Cargo,
                LanguageId::Rust,
                PathBuf::from("Cargo.toml"),
            )
            .with_depth(0)
            .with_confidence(1.0)
            .with_workspace_root(true),
            DetectionStack::new(
                BuildSystemId::Cargo,
                LanguageId::Rust,
                PathBuf::from("crates/lib/Cargo.toml"),
            )
            .with_depth(2)
            .with_confidence(1.0)
            .with_workspace_root(false),
        ];
        let context = BootstrapContext::from_detections(detections, false, 50);

        let prompt = context.format_for_prompt();
        assert!(prompt.contains("Workspace Roots:"));
        assert!(prompt.contains("Cargo.toml"));
        assert!(prompt.contains("monorepo with multiple sub-projects"));
    }
}
