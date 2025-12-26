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
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let ruby_version = wolfi_index
            .get_latest_version("ruby")
            .unwrap_or_else(|| "ruby-3.3".to_string());

        // Extract ruby version number (e.g., "ruby-3.3" -> "3.3")
        let ruby_ver_num = ruby_version.trim_start_matches("ruby-");
        let bundler_package = format!("ruby{}-bundler", ruby_ver_num);

        // Check if the bundler package exists, fall back to just ruby if not
        let build_packages = if wolfi_index.has_package(&bundler_package) {
            vec![ruby_version.clone(), bundler_package]
        } else {
            vec![ruby_version.clone()]
        };

        BuildTemplate {
            build_packages,
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
