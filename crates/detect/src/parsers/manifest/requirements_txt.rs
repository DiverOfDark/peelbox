use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

pub struct RequirementsTxtParser;

impl ManifestParser for RequirementsTxtParser {
    fn filenames(&self) -> &[&str] {
        &["requirements.txt"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let deps: Vec<Dependency> = content
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#') && !l.starts_with('-'))
            .map(|l| {
                let name = l
                    .split(&['>', '<', '=', '~', '!', '['][..])
                    .next()
                    .unwrap_or(l)
                    .trim()
                    .to_string();
                Dependency {
                    name,
                    version: None,
                    scope: DepScope::Runtime,
                    is_internal: false,
                }
            })
            .collect();

        if deps.is_empty() {
            return None;
        }

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Python,
            build_system: BuildSystemId::Pip,
            runtime: RuntimeId::Python,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: vec![
                    "python".into(),
                    "pip".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["pip install --user --no-cache-dir -r requirements.txt".into()],
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
                entrypoint: None,
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(RequirementsTxtParser))
}
