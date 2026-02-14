use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const ZIG: LanguageId = LanguageId::new("zig");
const ZIG_BS: BuildSystemId = BuildSystemId::new("zig");
const NATIVE: RuntimeId = RuntimeId::new("native");

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
            language: ZIG,
            build_system: ZIG_BS,
            runtime: NATIVE,
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
