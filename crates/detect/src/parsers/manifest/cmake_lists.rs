use crate::traits::ManifestParser;
use crate::types::*;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

pub struct CMakeListsParser;

impl ManifestParser for CMakeListsParser {
    fn filenames(&self) -> &[&str] {
        &["CMakeLists.txt"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("cmake_minimum_required") && !content.contains("project(") {
            return None;
        }

        let name_re = regex::Regex::new(r"project\s*\(\s*(\w+)").ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Cpp,
            build_system: BuildSystemId::CMake,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "build-base".into(),
                    "cmake".into(),
                    "gcc".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "cmake -B build -DCMAKE_BUILD_TYPE=Release".into(),
                    "cmake --build build --config Release".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["CMakeCache.txt".into(), "build".into()],
                artifacts: vec![(format!("build/{}", name), format!("/app/{}", name))],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(CMakeListsParser))
}
