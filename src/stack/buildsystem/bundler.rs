//! Bundler build system (Ruby)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct BundlerBuildSystem;

impl BuildSystem for BundlerBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Bundler
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "Gemfile".to_string(),
            priority: 10,
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
            if path.file_name().and_then(|n| n.to_str()) == Some("Gemfile") {
                detections.push(DetectionStack::new(
                    BuildSystemId::Bundler,
                    LanguageId::Ruby,
                    path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let ruby_version = wolfi_index
            .get_latest_version("ruby")
            .unwrap_or_else(|| "ruby-3.3".to_string());

        BuildTemplate {
            build_packages: vec![ruby_version.clone()],
            runtime_packages: vec![ruby_version],
            build_commands: vec!["bundle install".to_string()],
            cache_paths: vec!["vendor/bundle/".to_string()],
            artifacts: vec![],
            common_ports: vec![3000],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["vendor".to_string(), ".bundle".to_string()]
    }
}
