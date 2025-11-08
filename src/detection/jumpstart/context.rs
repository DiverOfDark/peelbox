//! Jumpstart context generation for LLM prompts

use crate::detection::jumpstart::scanner::ManifestFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// Context generated from jumpstart scan for LLM consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JumpstartContext {
    /// List of discovered manifest files
    pub manifest_files: Vec<ManifestFile>,
    /// Detected project hints based on manifests
    pub project_hints: ProjectHints,
    /// Workspace structure analysis
    pub workspace_info: WorkspaceInfo,
    /// Total scan time in milliseconds
    pub scan_time_ms: u64,
}

/// Detected project hints from manifest analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectHints {
    /// Likely primary build system
    pub likely_build_system: Option<String>,
    /// Detected languages
    pub detected_languages: Vec<String>,
    /// Is this a monorepo/workspace?
    pub is_monorepo: bool,
    /// Manifest count by type
    pub manifest_counts: HashMap<String, usize>,
}

/// Workspace structure information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// Root-level manifests (depth 0)
    pub root_manifests: Vec<String>,
    /// Nested manifests by depth
    pub nested_manifests: HashMap<usize, Vec<String>>,
    /// Total depth of discovered manifests
    pub max_depth: usize,
}

impl JumpstartContext {
    /// Creates a new context from discovered manifest files
    pub fn from_manifests(manifests: Vec<ManifestFile>, scan_time_ms: u64) -> Self {
        info!(
            manifest_count = manifests.len(),
            scan_time_ms, "Generating jumpstart context"
        );

        let project_hints = Self::analyze_project_hints(&manifests);
        let workspace_info = Self::analyze_workspace(&manifests);

        debug!(
            likely_build_system = ?project_hints.likely_build_system,
            detected_languages = ?project_hints.detected_languages,
            is_monorepo = project_hints.is_monorepo,
            "Project analysis complete"
        );

        Self {
            manifest_files: manifests,
            project_hints,
            workspace_info,
            scan_time_ms,
        }
    }

    /// Analyzes manifests to generate project hints
    fn analyze_project_hints(manifests: &[ManifestFile]) -> ProjectHints {
        let mut manifest_counts: HashMap<String, usize> = HashMap::new();
        let mut detected_languages = Vec::new();

        for manifest in manifests {
            *manifest_counts.entry(manifest.name.clone()).or_insert(0) += 1;
        }

        // Language detection
        if manifest_counts.contains_key("Cargo.toml") {
            detected_languages.push("Rust".to_string());
        }
        if manifest_counts.contains_key("package.json") {
            detected_languages.push("JavaScript/TypeScript".to_string());
        }
        if manifest_counts.contains_key("pom.xml")
            || manifest_counts.keys().any(|k| k.contains("gradle"))
        {
            detected_languages.push("Java".to_string());
        }
        if manifest_counts.contains_key("requirements.txt")
            || manifest_counts.contains_key("pyproject.toml")
            || manifest_counts.contains_key("Pipfile")
        {
            detected_languages.push("Python".to_string());
        }
        if manifest_counts.contains_key("go.mod") {
            detected_languages.push("Go".to_string());
        }
        if manifest_counts.contains_key("Gemfile") {
            detected_languages.push("Ruby".to_string());
        }
        if manifest_counts.contains_key("composer.json") {
            detected_languages.push("PHP".to_string());
        }
        if manifest_counts
            .keys()
            .any(|k| k.ends_with(".csproj") || k.ends_with(".sln"))
        {
            detected_languages.push(".NET".to_string());
        }

        // Build system detection (primary)
        let likely_build_system = Self::detect_build_system(&manifest_counts);

        // Monorepo detection
        let is_monorepo = Self::detect_monorepo(manifests, &manifest_counts);

        ProjectHints {
            likely_build_system,
            detected_languages,
            is_monorepo,
            manifest_counts,
        }
    }

    /// Detects the likely primary build system
    fn detect_build_system(manifest_counts: &HashMap<String, usize>) -> Option<String> {
        // Priority order for build system detection
        if manifest_counts.contains_key("Cargo.toml") {
            Some("Cargo".to_string())
        } else if manifest_counts.contains_key("package.json") {
            Some("npm/yarn/pnpm".to_string())
        } else if manifest_counts.contains_key("pom.xml") {
            Some("Maven".to_string())
        } else if manifest_counts.keys().any(|k| k.contains("gradle")) {
            Some("Gradle".to_string())
        } else if manifest_counts.contains_key("go.mod") {
            Some("Go modules".to_string())
        } else if manifest_counts.contains_key("pyproject.toml") {
            Some("Poetry".to_string())
        } else if manifest_counts.contains_key("Pipfile") {
            Some("Pipenv".to_string())
        } else if manifest_counts.contains_key("requirements.txt") {
            Some("pip".to_string())
        } else if manifest_counts.contains_key("Gemfile") {
            Some("Bundler".to_string())
        } else if manifest_counts.contains_key("composer.json") {
            Some("Composer".to_string())
        } else if manifest_counts.contains_key("Makefile") {
            Some("Make".to_string())
        } else {
            None
        }
    }

    /// Detects if this is a monorepo/workspace
    fn detect_monorepo(
        manifests: &[ManifestFile],
        manifest_counts: &HashMap<String, usize>,
    ) -> bool {
        // Check for workspace configuration files
        let has_workspace_config = manifest_counts.contains_key("pnpm-workspace.yaml")
            || manifest_counts.contains_key("lerna.json")
            || manifest_counts.contains_key("nx.json")
            || manifest_counts.contains_key("turbo.json")
            || manifest_counts.contains_key("rush.json");

        if has_workspace_config {
            return true;
        }

        // Check for multiple nested manifests of the same type
        let nested_manifests = manifests.iter().filter(|m| m.depth > 0).count();
        if nested_manifests > 2 {
            return true;
        }

        // Check for multiple package.json or Cargo.toml files
        if manifest_counts.get("package.json").copied().unwrap_or(0) > 1
            || manifest_counts.get("Cargo.toml").copied().unwrap_or(0) > 1
        {
            return true;
        }

        false
    }

    /// Analyzes workspace structure
    fn analyze_workspace(manifests: &[ManifestFile]) -> WorkspaceInfo {
        let mut root_manifests = Vec::new();
        let mut nested_manifests: HashMap<usize, Vec<String>> = HashMap::new();
        let mut max_depth = 0;

        for manifest in manifests {
            if manifest.depth == 0 {
                root_manifests.push(manifest.name.clone());
            } else {
                nested_manifests
                    .entry(manifest.depth)
                    .or_default()
                    .push(manifest.path.clone());
                max_depth = max_depth.max(manifest.depth);
            }
        }

        WorkspaceInfo {
            root_manifests,
            nested_manifests,
            max_depth,
        }
    }

    /// Formats the context as a compact JSON string for LLM prompts
    pub fn to_prompt_string(&self) -> String {
        let manifest_list: Vec<&str> = self
            .manifest_files
            .iter()
            .map(|m| m.path.as_str())
            .collect();

        let hints = &self.project_hints;

        format!(
            r#"Pre-scanned repository manifest files ({}):
Files: {}

Project Hints:
- Likely build system: {}
- Detected languages: {}
- Monorepo: {}
- Root manifests: {}

Use these files to guide your analysis. You can read any of these files directly without searching."#,
            manifest_list.len(),
            serde_json::to_string(&manifest_list).unwrap_or_else(|_| "[]".to_string()),
            hints
                .likely_build_system
                .as_ref()
                .unwrap_or(&"unknown".to_string()),
            hints.detected_languages.join(", "),
            hints.is_monorepo,
            self.workspace_info.root_manifests.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manifests() -> Vec<ManifestFile> {
        vec![
            ManifestFile {
                path: "Cargo.toml".to_string(),
                name: "Cargo.toml".to_string(),
                depth: 0,
            },
            ManifestFile {
                path: "package.json".to_string(),
                name: "package.json".to_string(),
                depth: 0,
            },
            ManifestFile {
                path: "subproject/Cargo.toml".to_string(),
                name: "Cargo.toml".to_string(),
                depth: 1,
            },
        ]
    }

    #[test]
    fn test_context_creation() {
        let manifests = create_test_manifests();
        let context = JumpstartContext::from_manifests(manifests, 100);

        assert_eq!(context.manifest_files.len(), 3);
        assert_eq!(context.scan_time_ms, 100);
    }

    #[test]
    fn test_language_detection() {
        let manifests = create_test_manifests();
        let context = JumpstartContext::from_manifests(manifests, 100);

        assert!(context
            .project_hints
            .detected_languages
            .contains(&"Rust".to_string()));
        assert!(context
            .project_hints
            .detected_languages
            .contains(&"JavaScript/TypeScript".to_string()));
    }

    #[test]
    fn test_build_system_detection() {
        let manifests = vec![ManifestFile {
            path: "Cargo.toml".to_string(),
            name: "Cargo.toml".to_string(),
            depth: 0,
        }];
        let context = JumpstartContext::from_manifests(manifests, 100);

        assert_eq!(
            context.project_hints.likely_build_system,
            Some("Cargo".to_string())
        );
    }

    #[test]
    fn test_monorepo_detection() {
        let manifests = vec![
            ManifestFile {
                path: "package.json".to_string(),
                name: "package.json".to_string(),
                depth: 0,
            },
            ManifestFile {
                path: "packages/app/package.json".to_string(),
                name: "package.json".to_string(),
                depth: 2,
            },
        ];
        let context = JumpstartContext::from_manifests(manifests, 100);

        assert!(context.project_hints.is_monorepo);
    }

    #[test]
    fn test_workspace_detection_with_config() {
        let manifests = vec![
            ManifestFile {
                path: "pnpm-workspace.yaml".to_string(),
                name: "pnpm-workspace.yaml".to_string(),
                depth: 0,
            },
            ManifestFile {
                path: "package.json".to_string(),
                name: "package.json".to_string(),
                depth: 0,
            },
        ];
        let context = JumpstartContext::from_manifests(manifests, 100);

        assert!(context.project_hints.is_monorepo);
    }

    #[test]
    fn test_workspace_info() {
        let manifests = create_test_manifests();
        let context = JumpstartContext::from_manifests(manifests, 100);

        assert_eq!(context.workspace_info.root_manifests.len(), 2);
        assert!(context
            .workspace_info
            .root_manifests
            .contains(&"Cargo.toml".to_string()));
        assert!(context
            .workspace_info
            .root_manifests
            .contains(&"package.json".to_string()));
        assert_eq!(context.workspace_info.max_depth, 1);
    }

    #[test]
    fn test_to_prompt_string() {
        let manifests = create_test_manifests();
        let context = JumpstartContext::from_manifests(manifests, 100);

        let prompt = context.to_prompt_string();

        assert!(prompt.contains("Pre-scanned repository"));
        assert!(prompt.contains("Cargo.toml"));
        assert!(prompt.contains("package.json"));
        assert!(prompt.contains("Likely build system"));
        assert!(prompt.contains("Detected languages"));
    }
}
