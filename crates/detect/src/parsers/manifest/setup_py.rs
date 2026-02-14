use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const PYTHON: LanguageId = LanguageId::new("python");
const PIP: BuildSystemId = BuildSystemId::new("pip");
const PYTHON_RT: RuntimeId = RuntimeId::new("python");

pub struct SetupPyParser;

impl ManifestParser for SetupPyParser {
    fn filenames(&self) -> &[&str] {
        &["setup.py"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("setup(") {
            return None;
        }

        let name_re = regex::Regex::new(r#"name\s*=\s*['"]([^'"]+)['"]"#).ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: PYTHON,
            build_system: PIP,
            runtime: PYTHON_RT,
            package: name.as_ref().map(|n| Package {
                name: n.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "python".into(),
                    "pip".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["pip install --user --no-cache-dir .".into()],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".cache/pip".into()],
                artifacts: vec![(".".into(), "/app/".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "python".into(),
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: name.map(|n| format!("python -m {}", n.replace('-', "_"))),
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(SetupPyParser))
}
