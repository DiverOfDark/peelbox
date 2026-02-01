use std::path::{Path, PathBuf};

use peelbox_core::fs::FileSystem;

use crate::{BuildSystemId, LanguageId};

use super::{DetectionResult, LanguageDefinition};

pub struct DenoLanguage;

impl LanguageDefinition for DenoLanguage {
    fn id(&self) -> LanguageId {
        LanguageId::Deno
    }

    fn extensions(&self) -> Vec<String> {
        vec![
            "ts".to_string(),
            "tsx".to_string(),
            "js".to_string(),
            "jsx".to_string(),
        ]
    }

    fn detect(
        &self,
        manifest_name: &str,
        _manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        if manifest_name == "deno.json" || manifest_name == "deno.jsonc" {
            Some(DetectionResult {
                build_system: BuildSystemId::Deno,
                confidence: 1.0,
            })
        } else {
            None
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["Deno".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec!["vendor".to_string(), ".deno".to_string()]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec!["deno.json".to_string(), "deno.jsonc".to_string()]
    }

    fn is_workspace_root(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "deno.json" && manifest_name != "deno.jsonc" {
            return false;
        }

        if let Some(content) = manifest_content {
            if content.contains("\"workspace\"") || content.contains("\"members\"") {
                return true;
            }
        }
        false
    }

    fn is_main_file(&self, _fs: &dyn FileSystem, file_path: &Path) -> bool {
        if let Some(name) = file_path.file_name().and_then(|n| n.to_str()) {
            return matches!(
                name,
                "main.ts" | "index.ts" | "mod.ts" | "server.ts" | "app.ts"
            );
        }
        false
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("main.ts".to_string())
    }

    fn is_runnable(
        &self,
        fs: &dyn FileSystem,
        _repo_root: &Path,
        project_root: &Path,
        _file_tree: &[PathBuf],
        _manifest_content: Option<&str>,
    ) -> bool {
        for name in &["main.ts", "index.ts", "mod.ts", "server.ts", "app.ts"] {
            if fs.exists(&project_root.join(name)) {
                return true;
            }
        }
        false
    }

    fn runtime_name(&self) -> Option<String> {
        Some("Deno".to_string())
    }
}
