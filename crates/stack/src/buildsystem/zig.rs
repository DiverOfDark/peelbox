use super::BuildSystem;
use crate::{BuildSystemId, BuildTemplate, DetectionStack, ManifestPattern};
use peelbox_core::fs::FileSystem;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct ZigBuildSystem;

impl BuildSystem for ZigBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Zig
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "build.zig".to_string(),
            priority: 100,
        }]
    }

    fn detect_all(
        &self,
        _repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> anyhow::Result<Vec<DetectionStack>> {
        let mut results = Vec::new();
        let mut processed_dirs = HashSet::new();

        for path in file_tree {
            if path.file_name().and_then(|n| n.to_str()) == Some("build.zig") {
                let dir = path.parent().unwrap_or(Path::new(""));

                if processed_dirs.contains(dir) {
                    continue;
                }
                processed_dirs.insert(dir.to_path_buf());

                results.push(DetectionStack {
                    language: crate::LanguageId::Zig,
                    build_system: self.id(),
                    framework: None,
                    is_workspace_root: false,
                    manifest_path: path.clone(),
                    confidence: 1.0,
                    depth: 0,
                });
            }
        }

        Ok(results)
    }

    fn build_template(
        &self,
        _wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _relative_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        BuildTemplate {
            build_packages: vec!["zig".to_string(), "build-base".to_string()],
            build_commands: vec!["zig build -Doptimize=ReleaseSafe".to_string()],
            build_env: Default::default(),
            cache_paths: vec![
                ".zig-cache".to_string(),
                "zig-out".to_string(),
                "zig-cache".to_string(),
            ],
            common_ports: vec![],
            runtime_copy: vec![("zig-out/bin".to_string(), "/app".to_string())],
            runtime_env: Default::default(),
            runtime_workdir: Some("/app/".to_string()),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["zig-cache".to_string(), "zig-out".to_string()]
    }
}
