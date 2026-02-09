use crate::helpers::btree;
use crate::traits::ManifestParser;
use crate::types::*;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use std::collections::BTreeMap;
use std::path::Path;

pub struct PyProjectTomlParser;

impl ManifestParser for PyProjectTomlParser {
    fn filenames(&self) -> &[&str] {
        &["pyproject.toml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let toml_val: toml::Value = toml::from_str(content).ok()?;

        let is_poetry = toml_val
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .is_some();

        let (name, version) = if is_poetry {
            let poetry = toml_val.get("tool").and_then(|t| t.get("poetry"));
            let name = poetry
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let version = poetry
                .and_then(|p| p.get("version"))
                .and_then(|v| v.as_str())
                .map(String::from);
            (name, version)
        } else {
            let project = toml_val.get("project");
            let name = project
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .map(String::from);
            let version = project
                .and_then(|p| p.get("version"))
                .and_then(|v| v.as_str())
                .map(String::from);
            (name, version)
        };

        let build_system_id = if is_poetry {
            BuildSystemId::Poetry
        } else {
            BuildSystemId::Pip
        };

        let dependencies = parse_pyproject_deps(&toml_val, is_poetry);

        if is_poetry {
            // Poetry-specific build
            Some(Manifest {
                path: path.to_path_buf(),
                language: LanguageId::Python,
                build_system: build_system_id,
                runtime: RuntimeId::Python,
                package: Some(Package {
                    name: "app".to_string(),
                    version,
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
                    commands: vec![
                        "pip install --user poetry".into(),
                        "/root/.local/bin/poetry install --no-root --only main".into(),
                    ],
                    member_transform: None,
                    env: btree(&[
                        ("POETRY_CACHE_DIR", "/root/.cache/pypoetry"),
                        ("POETRY_VIRTUALENVS_IN_PROJECT", "true"),
                    ]),
                    cache_dirs: vec![
                        "/root/.cache/pip/".into(),
                        "/root/.cache/pypoetry/".into(),
                    ],
                    artifacts: vec![(".".into(), "/build".into())],
                },
                runtime_config: RuntimeSpec {
                    packages: vec![
                        "python".into(),
                        "libgcc".into(),
                        "libstdc++".into(),
                        "ca-certificates".into(),
                    ],
                    env: BTreeMap::new(),
                    entrypoint: None, // Will be set by framework detector (Flask)
                    workdir: Some("/build".into()),
                    ports: vec![8000],
                    health_endpoint: None,
                },
            })
        } else {
            Some(Manifest {
                path: path.to_path_buf(),
                language: LanguageId::Python,
                build_system: build_system_id,
                runtime: RuntimeId::Python,
                package: name.as_ref().map(|n| Package {
                    name: n.clone(),
                    version,
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
                    entrypoint: name
                        .as_ref()
                        .map(|n| format!("python -m {}", n.replace('-', "_"))),
                    workdir: Some("/app".into()),
                    ports: vec![8000],
                    health_endpoint: None,
                },
            })
        }
    }
}

fn parse_pyproject_deps(toml_val: &toml::Value, is_poetry: bool) -> Vec<Dependency> {
    let mut deps = Vec::new();

    if is_poetry {
        if let Some(poetry_deps) = toml_val
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("dependencies"))
            .and_then(|d| d.as_table())
        {
            for (name, val) in poetry_deps {
                if name == "python" {
                    continue;
                }
                let version = match val {
                    toml::Value::String(s) => Some(s.clone()),
                    toml::Value::Table(t) => t
                        .get("version")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    _ => None,
                };
                deps.push(Dependency {
                    name: name.clone(),
                    version,
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
        }
    } else if let Some(dep_list) = toml_val
        .get("project")
        .and_then(|p| p.get("dependencies"))
        .and_then(|d| d.as_array())
    {
        for dep in dep_list {
            if let Some(dep_str) = dep.as_str() {
                let name = dep_str
                    .split(&['>', '<', '=', '~', '!', '['][..])
                    .next()
                    .unwrap_or(dep_str)
                    .trim()
                    .to_string();
                deps.push(Dependency {
                    name,
                    version: None,
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
        }
    }

    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PyProjectTomlParser))
}
