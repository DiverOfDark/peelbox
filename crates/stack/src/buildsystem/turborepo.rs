use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

pub struct TurborepoBuildSystem;

impl BuildSystem for TurborepoBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Npm
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "turbo.json".to_string(),
            priority: 20,
        }]
    }

    fn detect_all(
        &self,
        _repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut detections = Vec::new();

        for path in file_tree {
            if path.file_name().and_then(|n| n.to_str()) == Some("turbo.json") {
                detections.push(DetectionStack::new(
                    BuildSystemId::Npm,
                    LanguageId::TypeScript,
                    path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _relative_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let node_version = wolfi_index
            .get_latest_version("nodejs")
            .expect("Failed to get nodejs version from Wolfi index");

        BuildTemplate {
            build_packages: vec![node_version, "npm".to_string(), "git".to_string()],
            build_commands: vec!["npm install".to_string(), "npx turbo build".to_string()],
            cache_paths: vec![".turbo".to_string()],
            common_ports: vec![3000],
            build_env: std::collections::BTreeMap::new(),
            runtime_copy: vec![
                ("package.json".to_string(), "/app/package.json".to_string()),
                ("dist/".to_string(), "/app/dist/".to_string()),
                (
                    "node_modules/".to_string(),
                    "/app/node_modules/".to_string(),
                ),
            ],
            runtime_workdir: Some("/app/".to_string()),
            runtime_env: std::collections::BTreeMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec![".turbo".to_string()]
    }

    fn is_workspace_root(&self, _manifest_content: Option<&str>) -> bool {
        true
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec!["turbo.json".to_string()]
    }
}
