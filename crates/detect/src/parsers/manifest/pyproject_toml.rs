use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const PYTHON: LanguageId = LanguageId::new("python");
const POETRY: BuildSystemId = BuildSystemId::new("poetry");
const PIP: BuildSystemId = BuildSystemId::new("pip");
const PYTHON_RT: RuntimeId = RuntimeId::new("python");

inventory::submit! {
    LanguageMeta { slug: "python", display_name: "Python", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "poetry", display_name: "Poetry", aliases: &["poetry"] }
}
inventory::submit! {
    BuildSystemMeta { slug: "pip", display_name: "pip", aliases: &[] }
}
inventory::submit! {
    RuntimeMeta { slug: "python", display_name: "Python", aliases: &["python"] }
}

pub struct PyProjectTomlParser;

impl ManifestParser for PyProjectTomlParser {
    fn filenames(&self) -> &[&str] {
        &["pyproject.toml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let toml_val: toml::Value = toml::from_str(content).ok()?;

        let is_poetry = toml_val.get("tool").and_then(|t| t.get("poetry")).is_some();

        // Detect Python version from requires-python or poetry python dep
        let python_version = extract_python_version(content, &toml_val, is_poetry);
        let python_build_pkg = python_version
            .as_ref()
            .map(|v| format!("python-{}", v))
            .unwrap_or_else(|| "python".into());
        let python_runtime_pkg = python_build_pkg.clone();

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

        let build_system_id = if is_poetry { POETRY } else { PIP };

        let dependencies = parse_pyproject_deps(&toml_val, is_poetry);

        if is_poetry {
            // Poetry-specific build
            Some(Manifest {
                path: path.to_path_buf(),
                language: PYTHON,
                build_system: build_system_id,
                runtime: PYTHON_RT,
                package: Some(Package {
                    name: "app".to_string(),
                    version,
                    is_application: true,
                }),
                workspace: None,
                dependencies,
                build: BuildSpec {
                    packages: vec![
                        python_build_pkg.clone(),
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
                    cache_dirs: vec!["/root/.cache/pip/".into(), "/root/.cache/pypoetry/".into()],
                    artifacts: vec![(".".into(), "/build".into())],
                },
                runtime_config: RuntimeSpec {
                    packages: vec![
                        python_runtime_pkg,
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
                language: PYTHON,
                build_system: build_system_id,
                runtime: PYTHON_RT,
                package: name.as_ref().map(|n| Package {
                    name: n.clone(),
                    version,
                    is_application: true,
                }),
                workspace: None,
                dependencies,
                build: BuildSpec {
                    packages: vec![
                        python_build_pkg,
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
                        python_runtime_pkg,
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
                    toml::Value::Table(t) => {
                        t.get("version").and_then(|v| v.as_str()).map(String::from)
                    }
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

/// Extract Python major.minor version from pyproject.toml content.
/// Only extracts exact/pinned versions (e.g., "==3.11"), not constraint-based ones
/// (e.g., "^3.9", ">=3.10") — those are minimum requirements that should be resolved
/// by Wolfi to the latest available version.
fn extract_python_version(
    content: &str,
    toml_val: &toml::Value,
    is_poetry: bool,
) -> Option<String> {
    // Check requires-python for exact version (PEP 621)
    // Only match "==3.11" or "3.11" (no constraint prefix), not ">=3.9" or "^3.9"
    if let Some(ver) = regex::Regex::new(r#"requires-python\s*=\s*"==(\d+\.\d+)"#)
        .ok()
        .and_then(|re| re.captures(content))
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
    {
        return Some(ver);
    }

    // Check Poetry python dependency — only exact versions
    if is_poetry {
        if let Some(python_constraint) = toml_val
            .get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("dependencies"))
            .and_then(|d| d.get("python"))
            .and_then(|v| v.as_str())
        {
            let trimmed = python_constraint.trim();
            // Only use exact versions like "3.11" or "3.11.4" (no constraint operators)
            if !trimmed.starts_with('^')
                && !trimmed.starts_with('~')
                && !trimmed.starts_with('>')
                && !trimmed.starts_with('<')
                && !trimmed.starts_with('!')
            {
                let re = regex::Regex::new(r"^(\d+\.\d+)").ok()?;
                if let Some(cap) = re.captures(trimmed) {
                    return cap.get(1).map(|m| m.as_str().to_string());
                }
            }
        }
    }

    // Check project.requires-python in TOML structure — only exact versions
    if let Some(requires) = toml_val
        .get("project")
        .and_then(|p| p.get("requires-python"))
        .and_then(|v| v.as_str())
    {
        let trimmed = requires.trim();
        if trimmed.starts_with("==") {
            let re = regex::Regex::new(r"==(\d+\.\d+)").ok()?;
            if let Some(cap) = re.captures(trimmed) {
                return cap.get(1).map(|m| m.as_str().to_string());
            }
        }
    }

    None
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PyProjectTomlParser))
}
