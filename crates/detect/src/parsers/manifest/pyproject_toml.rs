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
const PDM: BuildSystemId = BuildSystemId::new("pdm");
const PIP: BuildSystemId = BuildSystemId::new("pip");
const PYTHON_RT: RuntimeId = RuntimeId::new("python");

inventory::submit! {
    LanguageMeta { slug: "python", display_name: "Python", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "poetry", display_name: "Poetry", aliases: &["poetry"] }
}
inventory::submit! {
    BuildSystemMeta { slug: "pdm", display_name: "PDM", aliases: &["pdm"] }
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
        let is_pdm = toml_val.get("tool").and_then(|t| t.get("pdm")).is_some();

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
            // PDM and pip both use PEP 621 [project] metadata
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
            POETRY
        } else if is_pdm {
            PDM
        } else {
            PIP
        };

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
                        "pip install poetry".into(),
                        "poetry install --no-root --only main".into(),
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
        } else if is_pdm {
            // PDM-specific build
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
                        "pip install pdm".into(),
                        "pdm config venv.in-project true".into(),
                        "pdm install --no-self --prod".into(),
                    ],
                    member_transform: None,
                    env: btree(&[("PDM_PYTHON", "/usr/bin/python3")]),
                    cache_dirs: vec!["/root/.cache/pip/".into(), "/root/.cache/pdm/".into()],
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

// ── Build System Profiles ───────────────────────────────────────────────────

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        use_package_name_for_root: false,
        preferred_framework_env_keys: &["VIRTUAL_ENV"],
        ..BuildSystemConfig::new(POETRY)
    })
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        use_package_name_for_root: false,
        preferred_framework_env_keys: &["VIRTUAL_ENV"],
        ..BuildSystemConfig::new(PDM)
    })
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        use_package_name_for_root: false,
        ..BuildSystemConfig::new(PIP)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_pyproject_pdm_basic() {
        let parser = PyProjectTomlParser;
        let content = r#"
[project]
name = "my-pdm-app"
version = "0.1.0"
description = "A PDM project"
dependencies = [
    "flask>=3.0.0",
    "requests>=2.31.0",
]
requires-python = ">=3.9"

[tool.pdm]
distribution = false

[tool.pdm.dev-dependencies]
dev = ["pytest>=7.0"]

[build-system]
requires = ["pdm-backend"]
build-backend = "pdm.backend"
"#;
        let manifest = parser.parse(Path::new("pyproject.toml"), content).unwrap();
        assert_eq!(manifest.language, PYTHON);
        assert_eq!(manifest.build_system, PDM);
        assert_eq!(manifest.runtime, PYTHON_RT);

        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app");
        assert!(pkg.is_application);

        // Should have 2 runtime deps (flask, requests)
        assert_eq!(manifest.dependencies.len(), 2);
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "flask" && d.scope == DepScope::Runtime));
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "requests" && d.scope == DepScope::Runtime));

        // Check PDM build commands
        assert!(manifest.build.commands.iter().any(|c| c.contains("pdm")));
        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("pdm install")));

        // Check env vars
        assert!(manifest.build.env.contains_key("PDM_PYTHON"));

        // Check cache dirs include pdm cache
        assert!(manifest.build.cache_dirs.iter().any(|c| c.contains("pdm")));

        // Check artifacts
        assert_eq!(
            manifest.build.artifacts,
            vec![(".".into(), "/build".into())]
        );
    }

    #[test]
    fn test_pyproject_pdm_takes_priority_over_pip() {
        let parser = PyProjectTomlParser;
        // A pyproject.toml with both [project] and [tool.pdm] should be detected as PDM
        let content = r#"
[project]
name = "my-app"
version = "1.0.0"
dependencies = ["flask>=3.0"]

[tool.pdm]
distribution = false
"#;
        let manifest = parser.parse(Path::new("pyproject.toml"), content).unwrap();
        assert_eq!(
            manifest.build_system, PDM,
            "PDM should take priority over pip when [tool.pdm] is present"
        );
    }

    #[test]
    fn test_pyproject_poetry_still_detected() {
        let parser = PyProjectTomlParser;
        let content = r#"
[tool.poetry]
name = "my-app"
version = "0.1.0"

[tool.poetry.dependencies]
python = "^3.9"
flask = "^3.0.0"

[build-system]
requires = ["poetry-core"]
build-backend = "poetry.core.masonry.api"
"#;
        let manifest = parser.parse(Path::new("pyproject.toml"), content).unwrap();
        assert_eq!(
            manifest.build_system, POETRY,
            "Poetry should still be detected correctly"
        );
    }

    #[test]
    fn test_pyproject_plain_pip_still_detected() {
        let parser = PyProjectTomlParser;
        let content = r#"
[project]
name = "my-app"
version = "1.0.0"
dependencies = ["flask>=3.0"]

[build-system]
requires = ["setuptools"]
build-backend = "setuptools.build_meta"
"#;
        let manifest = parser.parse(Path::new("pyproject.toml"), content).unwrap();
        assert_eq!(
            manifest.build_system, PIP,
            "Plain pyproject.toml without [tool.pdm] or [tool.poetry] should be pip"
        );
    }

    #[test]
    fn test_pyproject_pdm_workdir_is_build() {
        let parser = PyProjectTomlParser;
        let content = r#"
[project]
name = "my-app"
version = "1.0.0"
dependencies = ["flask>=3.0"]

[tool.pdm]
distribution = false
"#;
        let manifest = parser.parse(Path::new("pyproject.toml"), content).unwrap();
        assert_eq!(manifest.runtime_config.workdir, Some("/build".into()));
    }
}
