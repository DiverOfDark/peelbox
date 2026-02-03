//! Zig language definition

use super::{DetectionResult, LanguageDefinition};

pub struct ZigLanguage;

impl LanguageDefinition for ZigLanguage {
    fn id(&self) -> crate::LanguageId {
        crate::LanguageId::Zig
    }

    fn extensions(&self) -> Vec<String> {
        vec!["zig".to_string()]
    }

    fn detect(
        &self,
        manifest_name: &str,
        _manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "build.zig" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Zig,
                confidence: 1.0,
            }),
            _ => None,
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["zig".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec!["zig-cache".to_string(), "zig-out".to_string()]
    }

    fn find_entrypoints(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        _repo_root: &std::path::Path,
        project_root: &std::path::Path,
        file_tree: &[std::path::PathBuf],
    ) -> Vec<String> {
        use regex::Regex;

        let main_pattern = Regex::new(r"pub\s+fn\s+main\s*\(").unwrap();
        let mut entrypoints = Vec::new();

        for file_path in file_tree {
            if file_path.starts_with(project_root)
                && file_path.extension().and_then(|s| s.to_str()) == Some("zig")
            {
                if let Ok(content) = fs.read_to_string(file_path) {
                    if main_pattern.is_match(&content) {
                        if let Some(name) = file_path.file_name().and_then(|n| n.to_str()) {
                            entrypoints.push(name.to_string());
                        }
                    }
                }
            }
        }

        entrypoints
    }

    fn is_runnable(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        repo_root: &std::path::Path,
        project_root: &std::path::Path,
        file_tree: &[std::path::PathBuf],
        _manifest_content: Option<&str>,
    ) -> bool {
        !self
            .find_entrypoints(fs, repo_root, project_root, file_tree)
            .is_empty()
    }
}
