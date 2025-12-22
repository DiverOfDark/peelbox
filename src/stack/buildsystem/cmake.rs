//! CMake build system (C++)

use super::{BuildSystem, BuildTemplate, ManifestPattern};

pub struct CMakeBuildSystem;

impl BuildSystem for CMakeBuildSystem {
    fn id(&self) -> crate::stack::BuildSystemId {
        crate::stack::BuildSystemId::CMake
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![ManifestPattern {
            filename: "CMakeLists.txt".to_string(),
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, _manifest_content: Option<&str>) -> bool {
        manifest_name == "CMakeLists.txt"
    }

    fn build_template(&self) -> BuildTemplate {
        BuildTemplate {
            build_image: "gcc:12".to_string(),
            runtime_image: "ubuntu:22.04".to_string(),
            build_packages: vec!["cmake".to_string(), "make".to_string()],
            runtime_packages: vec!["libstdc++6".to_string()],
            build_commands: vec![
                "cmake -B build -DCMAKE_BUILD_TYPE=Release".to_string(),
                "cmake --build build --config Release".to_string(),
            ],
            cache_paths: vec!["build/".to_string()],
            artifacts: vec!["build/{project_name}".to_string()],
            common_ports: vec![8080],
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["build".to_string(), "CMakeCache.txt".to_string()]
    }
}
