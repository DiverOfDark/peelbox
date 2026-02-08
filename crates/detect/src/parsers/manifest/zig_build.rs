use crate::traits::ManifestParser;
use crate::types::*;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

pub struct ZigBuildParser;

impl ManifestParser for ZigBuildParser {
    fn filenames(&self) -> &[&str] {
        &["build.zig"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("std.Build") && !content.contains("@import") {
            return None;
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Zig,
            build_system: BuildSystemId::Zig,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec!["zig".into(), "build-base".into(), "ca-certificates".into()],
                commands: vec!["zig build -Doptimize=ReleaseSafe".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["zig-cache".into(), "zig-out".into()],
                artifacts: vec![("zig-out/bin/*".into(), "/app/".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some("/app/app".into()),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(ZigBuildParser))
}
