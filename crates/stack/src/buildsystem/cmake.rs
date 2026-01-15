//! CMake build system (C++)

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

pub struct CMakeBuildSystem;

impl BuildSystem for CMakeBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::CMake
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "CMakeLists.txt".to_string(),
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
            if path.file_name().and_then(|n| n.to_str()) == Some("CMakeLists.txt") {
                detections.push(DetectionStack::new(
                    BuildSystemId::CMake,
                    LanguageId::Cpp,
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
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let mut build_packages = vec!["build-base".to_string()];

        if wolfi_index.has_package("cmake") {
            build_packages.push("cmake".to_string());
        }
        if wolfi_index.has_package("gcc") {
            build_packages.push("gcc".to_string());
        }

        BuildTemplate {
            build_packages,
            build_commands: vec![
                "cmake -B build -DCMAKE_BUILD_TYPE=Release".to_string(),
                "cmake --build build --config Release".to_string(),
            ],
            cache_paths: vec!["build/".to_string()],

            common_ports: vec![8080],
            build_env: std::collections::HashMap::new(),
            runtime_copy: vec![(
                "build/{project_name}".to_string(),
                "/usr/local/bin/{project_name}".to_string(),
            )],
            runtime_env: std::collections::HashMap::new(),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["build".to_string(), "CMakeCache.txt".to_string()]
    }
}
