//! Bundler build system (Ruby)

use super::ruby_common::{parse_gemfile_version, read_ruby_version_file};
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
        service_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let ruby_version = read_ruby_version_file(service_path)
            .or_else(|| manifest_content.and_then(|c| parse_gemfile_version(c)))
            .or_else(|| wolfi_index.get_latest_version("ruby"))
            .expect("Failed to get ruby version from Wolfi index");

        let ruby_ver_num = ruby_version.trim_start_matches("ruby-");
        let bundler_package = format!("ruby{}-bundler", ruby_ver_num);
        let ruby_dev_package = format!("{}-dev", ruby_version);

        let mut build_packages = vec![ruby_version.clone()];

        if wolfi_index.has_package(&ruby_dev_package) {
            build_packages.push(ruby_dev_package);
        }

        if wolfi_index.has_package(&bundler_package) {
            build_packages.push(bundler_package);
        }

        build_packages.push("build-base".to_string());

        let mut build_env = std::collections::HashMap::new();
        build_env.insert("BUNDLE_PATH".to_string(), "vendor/bundle".to_string());
        build_env.insert("BUNDLE_DEPLOYMENT".to_string(), "false".to_string());

        let mut runtime_env = std::collections::HashMap::new();
        runtime_env.insert("BUNDLE_PATH".to_string(), "/app/vendor/bundle".to_string());
        runtime_env.insert("BUNDLE_GEMFILE".to_string(), "/app/Gemfile".to_string());

        BuildTemplate {
            build_packages,
            build_commands: vec!["bundle install".to_string()],
            cache_paths: vec!["vendor/bundle/".to_string()],
            
            common_ports: vec![3000],
            build_env,
            runtime_copy: vec![("/build/".to_string(), "/app".to_string())],
            runtime_env,
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["vendor".to_string(), ".bundle".to_string()]
    }
}
