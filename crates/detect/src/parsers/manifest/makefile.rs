use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

pub struct MakefileParser;

impl ManifestParser for MakefileParser {
    fn filenames(&self) -> &[&str] {
        &["Makefile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains(':') {
            return None;
        }

        let (language, runtime) = (LanguageId::Cpp, RuntimeId::Native);

        Some(Manifest {
            path: path.to_path_buf(),
            language,
            build_system: BuildSystemId::Make,
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
