//! Bootstrap context types for LLM prompt enrichment

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Complete bootstrap context for LLM prompt injection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapContext {
    /// Summary of repository structure
    pub summary: RepoSummary,
    /// Detected languages and build systems
    pub detections: Vec<LanguageDetection>,
    /// Workspace structure information
    pub workspace: WorkspaceInfo,
    /// Scan duration in milliseconds
    pub scan_time_ms: u64,
}

/// High-level repository summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    /// Total manifest files discovered
    pub manifest_count: usize,
    /// Primary detected language (highest confidence)
    pub primary_language: Option<String>,
    /// Primary build system
    pub primary_build_system: Option<String>,
    /// Whether this appears to be a monorepo
    pub is_monorepo: bool,
    /// Manifest files at root level
    pub root_manifests: Vec<String>,
}

/// A detected language/build system combination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageDetection {
    /// Language name
    pub language: String,
    /// Build system (cargo, npm, maven, etc.)
    pub build_system: String,
    /// Path to the manifest file
    pub manifest_path: String,
    /// Manifest file depth from root
    pub depth: usize,
    /// Detection confidence (0.0 - 1.0)
    pub confidence: f64,
}

/// Workspace structure analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// Root-level manifests
    pub root_manifests: Vec<String>,
    /// Nested manifests grouped by depth
    pub nested_by_depth: HashMap<usize, Vec<String>>,
    /// Maximum depth of manifests
    pub max_depth: usize,
    /// Has workspace configuration (pnpm-workspace, lerna, etc.)
    pub has_workspace_config: bool,
}

impl BootstrapContext {
    /// Creates a context from detected languages
    pub fn from_detections(
        detections: Vec<LanguageDetection>,
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
        detections: &[LanguageDetection],
        has_workspace_config: bool,
    ) -> WorkspaceInfo {
        let mut root_manifests = Vec::new();
        let mut nested_by_depth: HashMap<usize, Vec<String>> = HashMap::new();
        let mut max_depth = 0;

        for detection in detections {
            if detection.depth == 0 {
                root_manifests.push(detection.manifest_path.clone());
            } else {
                nested_by_depth
                    .entry(detection.depth)
                    .or_default()
                    .push(detection.manifest_path.clone());
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

    fn build_summary(detections: &[LanguageDetection], workspace: &WorkspaceInfo) -> RepoSummary {
        let primary = detections
            .iter()
            .filter(|d| d.depth == 0)
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap());

        let is_monorepo = workspace.has_workspace_config
            || workspace.max_depth > 1
            || detections.iter().filter(|d| d.depth > 0).count() > 2;

        RepoSummary {
            manifest_count: detections.len(),
            primary_language: primary.map(|d| d.language.clone()),
            primary_build_system: primary.map(|d| d.build_system.clone()),
            is_monorepo,
            root_manifests: workspace.root_manifests.clone(),
        }
    }

    /// Formats the context as a string for LLM system prompt injection
    pub fn format_for_prompt(&self) -> String {
        let manifest_list: Vec<&str> = self
            .detections
            .iter()
            .map(|d| d.manifest_path.as_str())
            .collect();

        let languages: Vec<String> = self
            .detections
            .iter()
            .filter(|d| d.depth == 0)
            .map(|d| format!("{} ({})", d.language, d.build_system))
            .collect();

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
- Root Manifests: {}

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
            self.summary.root_manifests.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_detections() -> Vec<LanguageDetection> {
        vec![
            LanguageDetection {
                language: "Rust".to_string(),
                build_system: "cargo".to_string(),
                manifest_path: "Cargo.toml".to_string(),
                depth: 0,
                confidence: 1.0,
            },
            LanguageDetection {
                language: "JavaScript".to_string(),
                build_system: "npm".to_string(),
                manifest_path: "package.json".to_string(),
                depth: 0,
                confidence: 0.8,
            },
            LanguageDetection {
                language: "Rust".to_string(),
                build_system: "cargo".to_string(),
                manifest_path: "crates/lib/Cargo.toml".to_string(),
                depth: 2,
                confidence: 1.0,
            },
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
            Some("cargo".to_string())
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
        let detections = vec![LanguageDetection {
            language: "JavaScript".to_string(),
            build_system: "npm".to_string(),
            manifest_path: "package.json".to_string(),
            depth: 0,
            confidence: 0.9,
        }];
        let context = BootstrapContext::from_detections(detections, true, 50);

        assert!(context.summary.is_monorepo);
    }

    #[test]
    fn test_workspace_info() {
        let detections = create_test_detections();
        let context = BootstrapContext::from_detections(detections, false, 50);

        assert_eq!(context.workspace.root_manifests.len(), 2);
        assert!(context.workspace.root_manifests.contains(&"Cargo.toml".to_string()));
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
        assert!(prompt.contains("cargo"));
    }

    #[test]
    fn test_empty_detections() {
        let context = BootstrapContext::from_detections(vec![], false, 10);

        assert_eq!(context.summary.manifest_count, 0);
        assert!(context.summary.primary_language.is_none());
        assert!(!context.summary.is_monorepo);
    }
}
