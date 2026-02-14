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

        let dependencies = parse_install_requires(content);

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
            dependencies,
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

/// Extract dependencies from install_requires=[...] in setup.py content.
fn parse_install_requires(content: &str) -> Vec<Dependency> {
    let re = regex::Regex::new(r"(?s)install_requires\s*=\s*\[(.*?)\]").ok();
    let Some(cap) = re.and_then(|r| r.captures(content)) else {
        return Vec::new();
    };

    let block = cap.get(1).map(|m| m.as_str()).unwrap_or("");
    let dep_re = regex::Regex::new(r#"['"]([^'"]+)['"]"#).unwrap();

    dep_re
        .captures_iter(block)
        .map(|c| {
            let raw = c.get(1).unwrap().as_str();
            let name = raw
                .split(&['>', '<', '=', '~', '!', '['][..])
                .next()
                .unwrap_or(raw)
                .trim()
                .to_string();
            Dependency {
                name,
                version: None,
                scope: DepScope::Runtime,
                is_internal: false,
            }
        })
        .collect()
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(SetupPyParser))
}
