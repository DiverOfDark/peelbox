use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const CPP: LanguageId = LanguageId::new("c++");
const MAKE: BuildSystemId = BuildSystemId::new("make");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    BuildSystemMeta { slug: "make", display_name: "Make", aliases: &["make"] }
}

pub struct MakefileParser;

impl ManifestParser for MakefileParser {
    fn filenames(&self) -> &[&str] {
        &["Makefile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains(':') {
            return None;
        }

        let (language, runtime) = (CPP, NATIVE);

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: MAKE,
            runtime,
            package: None,
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec!["make".into(), "build-base".into(), "ca-certificates".into()],
                commands: vec!["make".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: Vec::new(),
                artifacts: Vec::new(),
            },
            runtime_config: RuntimeSpec {
                packages: vec!["ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: None,
                workdir: Some("/app".into()),
                ports: Vec::new(),
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(MakefileParser))
}
