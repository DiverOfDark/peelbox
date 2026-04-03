use crate::helpers::{btree, extract_project_dir};
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use ignore::WalkBuilder;
use peelbox_core::output::schema::UniversalBuild;
use std::collections::BTreeMap;
use std::path::Path;
use tracing::debug;

const PYTHON: LanguageId = LanguageId::new("python");
const POETRY: BuildSystemId = BuildSystemId::new("poetry");
const PDM: BuildSystemId = BuildSystemId::new("pdm");
const PIP: BuildSystemId = BuildSystemId::new("pip");
pub(crate) const UV: BuildSystemId = BuildSystemId::new("uv");
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
    BuildSystemMeta { slug: "uv", display_name: "uv", aliases: &["uv"] }
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
        let is_uv = toml_val.get("tool").and_then(|t| t.get("uv")).is_some();
        let is_pdm = toml_val.get("tool").and_then(|t| t.get("pdm")).is_some();

        // Detect UV workspace members
        let uv_workspace = if is_uv {
            extract_uv_workspace(&toml_val)
        } else {
            None
        };

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

        // Extract first [project.scripts] entry (or [tool.poetry.scripts]) for entrypoint
        let script_entrypoint: Option<String> = if is_poetry {
            toml_val
                .get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("scripts"))
                .and_then(|s| s.as_table())
                .and_then(|t| t.keys().next())
                .map(|k| k.to_string())
        } else {
            toml_val
                .get("project")
                .and_then(|p| p.get("scripts"))
                .and_then(|s| s.as_table())
                .and_then(|t| t.keys().next())
                .map(|k| k.to_string())
        };

        let build_system_id = if is_poetry {
            POETRY
        } else if is_uv {
            UV
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
                        "pip install poetry poetry-plugin-export".into(),
                        // Ensure a lockfile exists (no-op if one is already present),
                        // then export to requirements.txt and install via pip.
                        // Poetry 2.x's modern installer rejects wheels whose ABI tags
                        // don't precisely match the target Python (common on Wolfi/Alpine).
                        // Using pip as the installer provides proper sdist fallback.
                        "poetry lock --no-update 2>/dev/null || poetry lock".into(),
                        "poetry export --without-hashes -f requirements.txt -o requirements.txt --only main".into(),
                        "python3 -m venv .venv && .venv/bin/pip install -r requirements.txt && sed -i \"1s|/build/.venv|/app/.venv|\" .venv/bin/* 2>/dev/null; ln -sf /usr/bin/python3 .venv/bin/python3 && ln -sf python3 .venv/bin/python".into(),
                    ],
                    member_transform: None,
                    env: btree(&[
                        ("POETRY_CACHE_DIR", "/root/.cache/pypoetry"),
                        ("POETRY_VIRTUALENVS_IN_PROJECT", "true"),
                    ]),
                    cache_dirs: vec!["/root/.cache/pip/".into(), "/root/.cache/pypoetry/".into()],
                    artifacts: vec![(".".into(), "/app".into())],
                build_image: None,
},
                runtime_config: RuntimeSpec {
                    packages: vec![
                        python_runtime_pkg,
                        "libgcc".into(),
                        "libstdc++".into(),
                        "ca-certificates".into(),
                    ],
                    env: btree(&[
                        ("PATH", "/app/.venv/bin:/usr/local/bin:/usr/bin:/bin"),
                        ("VIRTUAL_ENV", "/app/.venv"),
                    ]),
                    entrypoint: None, // Will be set by framework detector (Flask)
                    workdir: Some("/app".into()),
                    ports: vec![8000],
                    health_endpoint: None,
                },
            })
        } else if is_uv {
            // UV-specific build
            Some(Manifest {
                path: path.to_path_buf(),
                language: PYTHON,
                build_system: UV,
                runtime: PYTHON_RT,
                package: name.as_ref().map(|n| Package {
                    name: n.clone(),
                    version,
                    is_application: true,
                }),
                workspace: uv_workspace,
                dependencies,
                build: BuildSpec {
                    packages: vec![
                        python_build_pkg,
                        "pip".into(),
                        "build-base".into(),
                        "ca-certificates".into(),
                    ],
                    commands: vec![
                        "pip install --user uv".into(),
                        "/root/.local/bin/uv sync --frozen --no-dev".into(),
                    ],
                    member_transform: Some(MemberBuildTransform {
                        member_commands: vec![
                            "pip install --user uv".into(),
                            "/root/.local/bin/uv sync --frozen --no-dev --package {package}".into(),
                        ],
                        member_artifacts: None,
                    }),
                    env: btree(&[("UV_CACHE_DIR", "/root/.cache/uv")]),
                    cache_dirs: vec!["/root/.cache/pip/".into(), "/root/.cache/uv/".into()],
                    artifacts: vec![(".".into(), "/app".into())],
                    build_image: None,
                },
                runtime_config: RuntimeSpec {
                    packages: vec![
                        python_runtime_pkg,
                        "libgcc".into(),
                        "libstdc++".into(),
                        "ca-certificates".into(),
                    ],
                    env: btree(&[
                        ("PATH", "/app/.venv/bin:/usr/local/bin:/usr/bin:/bin"),
                        ("VIRTUAL_ENV", "/app/.venv"),
                    ]),
                    entrypoint: script_entrypoint.clone().or_else(|| {
                        name.as_ref()
                            .map(|n| format!("python -m {}", n.replace('-', "_")))
                    }),
                    workdir: Some("/app".into()),
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
                        "pdm export --no-hashes --prod -o requirements.txt".into(),
                        "python3 -m venv .venv && .venv/bin/pip install -r requirements.txt && sed -i \"1s|/build/.venv|/app/.venv|\" .venv/bin/* 2>/dev/null; ln -sf /usr/bin/python3 .venv/bin/python3 && ln -sf python3 .venv/bin/python".into(),
                    ],
                    member_transform: None,
                    env: btree(&[("PDM_PYTHON", "/usr/bin/python3")]),
                    cache_dirs: vec!["/root/.cache/pip/".into(), "/root/.cache/pdm/".into()],
                    artifacts: vec![(".".into(), "/app".into())],
                build_image: None,
},
                runtime_config: RuntimeSpec {
                    packages: vec![
                        python_runtime_pkg,
                        "libgcc".into(),
                        "libstdc++".into(),
                        "ca-certificates".into(),
                    ],
                    env: btree(&[
                        ("PATH", "/app/.venv/bin:/usr/local/bin:/usr/bin:/bin"),
                        ("VIRTUAL_ENV", "/app/.venv"),
                    ]),
                    entrypoint: None, // Will be set by framework detector (Flask)
                    workdir: Some("/app".into()),
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
                    artifacts: vec![
                        (".".into(), "/app/".into()),
                        ("/root/.local/".into(), "/root/.local/".into()),
                    ],
                    build_image: None,
                },
                runtime_config: RuntimeSpec {
                    packages: vec![
                        python_runtime_pkg,
                        "libgcc".into(),
                        "libstdc++".into(),
                        "ca-certificates".into(),
                    ],
                    env: btree(&[
                        ("PATH", "/root/.local/bin:/usr/local/bin:/usr/bin:/bin"),
                        ("PYTHONUSERBASE", "/root/.local"),
                    ]),
                    entrypoint: script_entrypoint.clone().or_else(|| {
                        name.as_ref()
                            .map(|n| format!("python -m {}", n.replace('-', "_")))
                    }),
                    workdir: Some("/app".into()),
                    ports: vec![8000],
                    health_endpoint: None,
                },
            })
        }
    }
}

/// Public wrapper for pdm_lock and other parsers that need to parse pyproject.toml deps.
pub fn parse_pyproject_deps_public(toml_val: &toml::Value, is_poetry: bool) -> Vec<Dependency> {
    parse_pyproject_deps(toml_val, is_poetry)
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

/// Extract UV workspace members from `[tool.uv.workspace]` section.
fn extract_uv_workspace(toml_val: &toml::Value) -> Option<Workspace> {
    let uv_workspace = toml_val
        .get("tool")
        .and_then(|t| t.get("uv"))
        .and_then(|u| u.get("workspace"));

    let members = uv_workspace
        .and_then(|ws| ws.get("members"))
        .and_then(|m| m.as_array())?;

    let member_patterns: Vec<String> = members
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    if member_patterns.is_empty() {
        return None;
    }

    Some(Workspace {
        members: member_patterns,
        orchestrator: None,
    })
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

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["Python"],
        extensions: &["py"],
        port_patterns: &[
            r"\.run\([^)]*port\s*=\s*(\d{4,5})",
            r#"port\s*=\s*(\d{4,5})"#,
        ],
        health_patterns: &[r#"@app\.(?:get|route)\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
        env_var_patterns: &[
            r#"os\.environ\.get\(['"]([A-Z_][A-Z0-9_]*)['"]"#,
            r#"os\.getenv\(['"]([A-Z_][A-Z0-9_]*)['"]"#,
            r#"os\.environ\[['"]([A-Z_][A-Z0-9_]*)['"]\]"#,
        ],
    }
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

// Note: UV build system profile is defined in uv_lock.rs

// ── Python version detection ────────────────────────────────────────────

/// Read Python version from `.python-version` file, `runtime.txt`, or `Pipfile`.
/// Returns the major.minor version string (e.g., "3.11").
pub(crate) fn read_python_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    // Check .python-version first (highest priority)
    for dir in &[project_dir, repo_root] {
        let path = dir.join(".python-version");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            // Extract major.minor (e.g., "3.11.4" -> "3.11")
            let parts: Vec<&str> = trimmed.split('.').collect();
            if parts.len() >= 2
                && parts[0].chars().all(|c| c.is_ascii_digit())
                && parts[1].chars().all(|c| c.is_ascii_digit())
            {
                return Some(format!("{}.{}", parts[0], parts[1]));
            }
        }
    }

    // Check runtime.txt (Heroku convention: "python-3.11.4" or "python-2.7")
    for dir in &[project_dir, repo_root] {
        let path = dir.join("runtime.txt");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim();
            if let Some(version_str) = trimmed.strip_prefix("python-") {
                let parts: Vec<&str> = version_str.split('.').collect();
                if parts.len() >= 2
                    && parts[0].chars().all(|c| c.is_ascii_digit())
                    && parts[1].chars().all(|c| c.is_ascii_digit())
                {
                    return Some(format!("{}.{}", parts[0], parts[1]));
                }
            }
        }
    }

    // Check Pipfile for python_version in [requires] section
    for dir in &[project_dir, repo_root] {
        let path = dir.join("Pipfile");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some(version) = parse_pipfile_python_version(&content) {
                return Some(version);
            }
        }
    }

    None
}

/// Extract `python_version` from Pipfile `[requires]` section.
pub(crate) fn parse_pipfile_python_version(content: &str) -> Option<String> {
    let re = regex::Regex::new(r#"(?m)^\s*python_version\s*=\s*["'](\d+\.\d+)["']"#).ok()?;
    re.captures(content)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

// ── Python entrypoint scanning ───────────────────────────────────────────

/// Common Python entrypoint filenames, ordered by priority.
pub(crate) const PYTHON_ENTRYPOINTS: &[&str] =
    &["app.py", "main.py", "server.py", "wsgi.py", "manage.py"];

/// Scan project directory for common Python entrypoint files.
/// Only overrides if current entrypoint is the fallback "python -m {name}".
pub(crate) fn scan_python_entrypoints(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    let is_fallback = build.runtime.command.is_empty()
        || (build.runtime.command.len() == 3
            && build.runtime.command[0] == "python"
            && build.runtime.command[1] == "-m");

    if !is_fallback {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    if !project_dir.is_dir() {
        return;
    }

    for filename in PYTHON_ENTRYPOINTS {
        if project_dir.join(filename).exists() {
            let workdir = &build.runtime.workdir;
            let entrypoint_path = format!("{}/{}", workdir, filename);
            debug!(entrypoint = %entrypoint_path, "Found Python entrypoint");
            build.runtime.command = vec!["python".into(), entrypoint_path];
            return;
        }
    }
}

// ── Django settings detection ────────────────────────────────────────────

/// Detect Django DJANGO_SETTINGS_MODULE from manage.py.
pub(crate) fn fix_django_settings(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }
    if build.metadata.framework.as_deref() != Some("Django") {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    let manage_py = project_dir.join("manage.py");
    if !manage_py.exists() {
        return;
    }

    if let Ok(content) = std::fs::read_to_string(&manage_py) {
        let re = regex::Regex::new(r#"DJANGO_SETTINGS_MODULE['"]\s*,\s*['"]([\w.]+)['"]"#).unwrap();
        if let Some(caps) = re.captures(&content) {
            let settings_module = caps.get(1).unwrap().as_str();
            build
                .runtime
                .env
                .insert("DJANGO_SETTINGS_MODULE".into(), settings_module.into());
        }
    }
}

// ── Python native dependency detection ──────────────────────────────────

/// Maps well-known Python packages to their required system build/runtime packages.
const PYTHON_NATIVE_DEPS: &[(&str, &[&str], &[&str])] = &[
    ("psycopg2", &["postgresql-dev"], &["libpq"]),
    ("psycopg2-binary", &["postgresql-dev"], &["libpq"]),
    ("psycopg", &["postgresql-dev"], &["libpq"]),
    (
        "mysqlclient",
        &["mariadb-connector-c-dev", "mariadb-connector-c"],
        &["mariadb-connector-c"],
    ),
    ("pycairo", &["cairo-dev"], &["cairo"]),
    ("pdf2image", &["poppler-utils"], &["poppler-utils"]),
    ("pydub", &["ffmpeg"], &["ffmpeg"]),
    (
        "Pillow",
        &["freetype-dev", "libjpeg-turbo-dev", "zlib-dev"],
        &["freetype", "libjpeg-turbo", "zlib"],
    ),
    (
        "pillow",
        &["freetype-dev", "libjpeg-turbo-dev", "zlib-dev"],
        &["freetype", "libjpeg-turbo", "zlib"],
    ),
    (
        "cryptography",
        &["openssl-dev", "libffi-dev"],
        &["openssl", "libffi"],
    ),
    (
        "lxml",
        &["libxml2-dev", "libxslt-dev"],
        &["libxml2", "libxslt"],
    ),
    ("cffi", &["libffi-dev"], &["libffi"]),
];

/// Scan Python dependencies and add required system packages for native extensions.
pub(crate) fn scan_python_native_deps(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    let dep_names = collect_python_dep_names(&project_dir);
    if dep_names.is_empty() {
        return;
    }

    let mut build_pkgs_to_add: Vec<String> = Vec::new();
    let mut runtime_pkgs_to_add: Vec<String> = Vec::new();

    for (pattern, build_deps, runtime_deps) in PYTHON_NATIVE_DEPS {
        if dep_names.iter().any(|d| dep_matches(d, pattern)) {
            for pkg in *build_deps {
                let pkg_str = pkg.to_string();
                if !build.build.packages.contains(&pkg_str) && !build_pkgs_to_add.contains(&pkg_str)
                {
                    build_pkgs_to_add.push(pkg_str);
                }
            }
            for pkg in *runtime_deps {
                let pkg_str = pkg.to_string();
                if !build.runtime.packages.contains(&pkg_str)
                    && !runtime_pkgs_to_add.contains(&pkg_str)
                {
                    runtime_pkgs_to_add.push(pkg_str);
                }
            }
        }
    }

    if !build_pkgs_to_add.is_empty() || !runtime_pkgs_to_add.is_empty() {
        if !build_pkgs_to_add.is_empty() {
            let dev_pkg = if let Some(py_pkg) = build
                .build
                .packages
                .iter()
                .find(|p| p.starts_with("python-3."))
            {
                Some(format!("{}-dev", py_pkg))
            } else if build.build.packages.iter().any(|p| p == "python") {
                Some("python-dev".to_string())
            } else {
                None
            };
            if let Some(dev) = dev_pkg {
                if !build.build.packages.contains(&dev) && !build_pkgs_to_add.contains(&dev) {
                    build_pkgs_to_add.push(dev);
                }
            }
        }

        debug!(
            build_pkgs = ?build_pkgs_to_add,
            runtime_pkgs = ?runtime_pkgs_to_add,
            "Adding system packages for Python native dependencies"
        );
        build.build.packages.extend(build_pkgs_to_add);
        build.runtime.packages.extend(runtime_pkgs_to_add);
    }
}

/// Check if a dependency name matches a pattern (case-insensitive, handles extras like `psycopg[c]`).
pub(crate) fn dep_matches(dep_name: &str, pattern: &str) -> bool {
    let normalized = dep_name.to_lowercase().replace('-', "_");
    let pat_normalized = pattern.to_lowercase().replace('-', "_");
    normalized == pat_normalized || normalized.starts_with(&format!("{}[", pat_normalized))
}

/// Collect Python dependency names from requirements.txt, pyproject.toml, Pipfile, and uv.lock.
pub(crate) fn collect_python_dep_names(project_dir: &Path) -> Vec<String> {
    let mut deps = Vec::new();

    if let Ok(content) = std::fs::read_to_string(project_dir.join("requirements.txt")) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }
            let name = trimmed
                .split(&['>', '<', '=', '~', '!', ';'][..])
                .next()
                .unwrap_or(trimmed)
                .trim();
            if !name.is_empty() {
                deps.push(name.to_string());
            }
        }
    }

    if let Ok(content) = std::fs::read_to_string(project_dir.join("pyproject.toml")) {
        if let Ok(toml_val) = toml::from_str::<toml::Value>(content.trim()) {
            if let Some(project_deps) = toml_val
                .get("project")
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_array())
            {
                for dep in project_deps {
                    if let Some(dep_str) = dep.as_str() {
                        let name = dep_str
                            .split(&['>', '<', '=', '~', '!', ';', ' '][..])
                            .next()
                            .unwrap_or(dep_str)
                            .trim();
                        if !name.is_empty() {
                            deps.push(name.to_string());
                        }
                    }
                }
            }

            if let Some(poetry_deps) = toml_val
                .get("tool")
                .and_then(|t| t.get("poetry"))
                .and_then(|p| p.get("dependencies"))
                .and_then(|d| d.as_table())
            {
                for name in poetry_deps.keys() {
                    if name != "python" {
                        deps.push(name.clone());
                    }
                }
            }
        }
    }

    if let Ok(content) = std::fs::read_to_string(project_dir.join("Pipfile")) {
        if let Ok(toml_val) = toml::from_str::<toml::Value>(content.trim()) {
            if let Some(packages) = toml_val.get("packages").and_then(|v| v.as_table()) {
                for name in packages.keys() {
                    deps.push(name.clone());
                }
            }
        }
    }

    deps
}

// ── Flask app path fix ────────────────────────────────────────────────────

/// Fix FLASK_APP for projects where the hardcoded `/app/app.py` doesn't exist.
pub(crate) fn fix_flask_app_path(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    let flask_app = match build.runtime.env.get("FLASK_APP") {
        Some(v) if v == "/app/app.py" => v.clone(),
        _ => return,
    };

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);
    let workdir = &build.runtime.workdir;

    if project_dir.join("app.py").exists() && project_dir == repo_root {
        return;
    }

    if project_dir != repo_root {
        if project_dir.join("app.py").exists() {
            let rel_path = project_dir
                .strip_prefix(repo_root)
                .unwrap_or(project_dir.as_path());
            let new_flask_app = format!("{}/{}/app.py", workdir, rel_path.display());
            debug!(old = %flask_app, new = %new_flask_app, "Fixed FLASK_APP for workspace member");
            build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
            return;
        }

        for entry in WalkBuilder::new(&project_dir)
            .max_depth(Some(4))
            .build()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "app.py" && entry.file_type().is_some_and(|t| t.is_file()) {
                let rel_path = entry.path().strip_prefix(repo_root).unwrap_or(entry.path());
                let new_flask_app = format!("{}/{}", workdir, rel_path.display());
                debug!(old = %flask_app, new = %new_flask_app, "Fixed FLASK_APP for workspace member");
                build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                return;
            }
        }
    }

    if project_dir.join("main.py").exists() {
        if let Ok(content) = std::fs::read_to_string(project_dir.join("main.py")) {
            if content.contains("from flask")
                || content.contains("import flask")
                || content.contains("import Flask")
                || content.contains("from Flask")
            {
                let workdir = &build.runtime.workdir;
                let new_flask_app = format!("{}/main.py", workdir);
                debug!(old = %flask_app, new = %new_flask_app, "Fixed FLASK_APP to main.py (contains Flask imports)");
                build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                return;
            }
        }
    }

    for entry in WalkBuilder::new(&project_dir)
        .max_depth(Some(4))
        .build()
        .filter_map(|e| e.ok())
    {
        if entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.ends_with(".py"))
            && entry.file_type().is_some_and(|t| t.is_file())
        {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                if content.contains("from flask") || content.contains("import flask") {
                    let rel_path = entry
                        .path()
                        .strip_prefix(&project_dir)
                        .unwrap_or(entry.path());
                    let workdir = &build.runtime.workdir;
                    if rel_path.file_name().is_some_and(|n| n == "__main__.py") {
                        if let Some(parent) = rel_path.parent() {
                            let dir_name = parent.to_string_lossy();
                            if !dir_name.is_empty() {
                                let abs_parent = project_dir.join(parent);
                                if abs_parent.join("__init__.py").exists() {
                                    let pkg_name = dir_name
                                        .replace('-', "_")
                                        .replace(std::path::MAIN_SEPARATOR, ".");
                                    let new_flask_app = format!("{}.__main__:app", pkg_name);
                                    build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                                    return;
                                }
                                debug!(
                                    path = %rel_path.display(),
                                    "Flask app in __main__.py without __init__.py, using direct Python execution"
                                );
                                let entrypoint = format!("{}/{}", workdir, rel_path.display());
                                build.runtime.env.remove("FLASK_APP");
                                build.runtime.env.remove("FLASK_RUN_HOST");
                                build.runtime.env.remove("FLASK_RUN_PORT");
                                build.runtime.command = vec!["python".into(), entrypoint];
                                return;
                            }
                        }
                    }
                    let new_flask_app = format!("{}/{}", workdir, rel_path.display());
                    build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                    return;
                }
            }
        }
    }

    debug!("No Flask app file found, falling back to Python entrypoint");
    build.runtime.env.remove("FLASK_APP");
    build.runtime.env.remove("FLASK_RUN_HOST");
    build.runtime.env.remove("FLASK_RUN_PORT");
    build.runtime.ports.clear();
    build.runtime.health = None;

    for filename in PYTHON_ENTRYPOINTS {
        if project_dir.join(filename).exists() {
            let workdir = &build.runtime.workdir;
            let entrypoint_path = format!("{}/{}", workdir, filename);
            debug!(entrypoint = %entrypoint_path, "Using Python entrypoint as Flask fallback");
            build.runtime.command = vec!["python".into(), entrypoint_path];
            return;
        }
    }
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
            .any(|c| c.contains("pdm export")));
        assert!(manifest.build.commands.iter().any(|c| c.contains(".venv")));

        // Check env vars
        assert!(manifest.build.env.contains_key("PDM_PYTHON"));

        // Check cache dirs include pdm cache
        assert!(manifest.build.cache_dirs.iter().any(|c| c.contains("pdm")));

        // Check artifacts
        assert_eq!(manifest.build.artifacts, vec![(".".into(), "/app".into())]);
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
    fn test_pyproject_pdm_workdir_is_app() {
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
        assert_eq!(manifest.runtime_config.workdir, Some("/app".into()));
    }

    // ── Python version detection tests ──────────────────────────────────

    #[test]
    fn test_parse_pipfile_python_version() {
        let content = r#"
[requires]
python_version = "3.11"
"#;
        assert_eq!(
            parse_pipfile_python_version(content),
            Some("3.11".to_string())
        );
    }

    #[test]
    fn test_parse_pipfile_python_version_single_quotes() {
        let content = r#"
[requires]
python_version = '3.10'
"#;
        assert_eq!(
            parse_pipfile_python_version(content),
            Some("3.10".to_string())
        );
    }

    #[test]
    fn test_parse_pipfile_no_version() {
        assert_eq!(
            parse_pipfile_python_version("[packages]\ndjango = \"*\"\n"),
            None
        );
    }

    #[test]
    fn test_read_python_version_from_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "3.11.4\n").unwrap();
        assert_eq!(
            read_python_version(dir.path(), dir.path()),
            Some("3.11".to_string())
        );
    }

    #[test]
    fn test_read_python_version_from_runtime_txt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("runtime.txt"), "python-3.10.12\n").unwrap();
        assert_eq!(
            read_python_version(dir.path(), dir.path()),
            Some("3.10".to_string())
        );
    }

    #[test]
    fn test_read_python_version_from_pipfile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Pipfile"),
            "[requires]\npython_version = \"3.9\"\n",
        )
        .unwrap();
        assert_eq!(
            read_python_version(dir.path(), dir.path()),
            Some("3.9".to_string())
        );
    }

    #[test]
    fn test_python_version_file_takes_priority() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".python-version"), "3.12.1\n").unwrap();
        std::fs::write(
            dir.path().join("Pipfile"),
            "[requires]\npython_version = \"3.9\"\n",
        )
        .unwrap();
        assert_eq!(
            read_python_version(dir.path(), dir.path()),
            Some("3.12".to_string())
        );
    }

    // ── Python native dependency detection tests ────────────────────────

    #[test]
    fn test_dep_matches_exact() {
        assert!(dep_matches("psycopg2", "psycopg2"));
        assert!(dep_matches("psycopg2-binary", "psycopg2-binary"));
        assert!(!dep_matches("psycopg2-binary", "psycopg2"));
        assert!(!dep_matches("psycopg2", "psycopg"));
    }

    #[test]
    fn test_dep_matches_extras() {
        assert!(dep_matches("psycopg[c]", "psycopg"));
        assert!(dep_matches("psycopg[binary]", "psycopg"));
        assert!(!dep_matches("psycopg2[binary]", "psycopg"));
    }

    #[test]
    fn test_dep_matches_case_insensitive() {
        assert!(dep_matches("Pillow", "pillow"));
        assert!(dep_matches("pillow", "Pillow"));
        assert!(dep_matches("Django", "django"));
    }

    #[test]
    fn test_dep_matches_underscore_hyphen() {
        assert!(dep_matches("psycopg2-binary", "psycopg2_binary"));
        assert!(dep_matches("psycopg2_binary", "psycopg2-binary"));
    }

    #[test]
    fn test_scan_python_native_deps_with_requirements() {
        use peelbox_core::output::schema::{BuildMetadata, BuildStage, RuntimeStage};

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("requirements.txt"),
            "psycopg2==2.9.3\nflask==3.0\n",
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("app".into()),
                language: "Python".into(),
                build_system: "pip".into(),
                framework: None,
                reasoning: "Detected from requirements.txt".into(),
            },
            build: BuildStage {
                packages: vec!["python-3.12".into(), "pip".into(), "build-base".into()],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec!["python-3.12".into(), "libgcc".into()],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["python".into(), "app.py".into()],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_python_native_deps(dir.path(), &mut build);

        assert!(
            build.build.packages.contains(&"postgresql-dev".to_string()),
            "Should add postgresql-dev for psycopg2"
        );
        assert!(
            build.runtime.packages.contains(&"libpq".to_string()),
            "Should add libpq for psycopg2 runtime"
        );
    }

    #[test]
    fn test_scan_python_native_deps_skips_non_python() {
        use peelbox_core::output::schema::{BuildMetadata, BuildStage, RuntimeStage};

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "psycopg2==2.9.3\n").unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("app".into()),
                language: "Rust".into(),
                build_system: "cargo".into(),
                framework: None,
                reasoning: "Detected from Cargo.toml".into(),
            },
            build: BuildStage {
                packages: vec!["rust".into()],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec![],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["./app".into()],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_python_native_deps(dir.path(), &mut build);

        assert!(
            !build.build.packages.contains(&"postgresql-dev".to_string()),
            "Should not add postgresql-dev for non-Python project"
        );
    }

    #[test]
    fn test_scan_python_native_deps_pyproject() {
        use peelbox_core::output::schema::{BuildMetadata, BuildStage, RuntimeStage};

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            r#"[project]
name = "myapp"
dependencies = [
    "mysqlclient>=2.1",
    "flask",
]
"#,
        )
        .unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("app".into()),
                language: "Python".into(),
                build_system: "pip".into(),
                framework: None,
                reasoning: "Detected from pyproject.toml".into(),
            },
            build: BuildStage {
                packages: vec!["python-3.12".into()],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec!["python-3.12".into()],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["python".into(), "app.py".into()],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_python_native_deps(dir.path(), &mut build);

        assert!(
            build
                .build
                .packages
                .contains(&"mariadb-connector-c-dev".to_string()),
            "Should add mariadb-connector-c-dev for mysqlclient"
        );
        assert!(
            build
                .runtime
                .packages
                .contains(&"mariadb-connector-c".to_string()),
            "Should add mariadb-connector-c for mysqlclient runtime"
        );
    }

    #[test]
    fn test_scan_python_native_deps_no_duplicates() {
        use peelbox_core::output::schema::{BuildMetadata, BuildStage, RuntimeStage};

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("requirements.txt"), "psycopg2==2.9.3\n").unwrap();

        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: BuildMetadata {
                project_name: Some("app".into()),
                language: "Python".into(),
                build_system: "pip".into(),
                framework: None,
                reasoning: "Detected from requirements.txt".into(),
            },
            build: BuildStage {
                packages: vec!["python-3.12".into(), "postgresql-dev".into()],
                env: BTreeMap::new(),
                commands: vec![],
                cache: vec![],
                build_image: None,
            },
            runtime: RuntimeStage {
                packages: vec!["python-3.12".into(), "libpq".into()],
                env: BTreeMap::new(),
                copy: vec![],
                command: vec!["python".into(), "app.py".into()],
                workdir: "/app".into(),
                ports: vec![],
                health: None,
            },
        };

        scan_python_native_deps(dir.path(), &mut build);

        let pg_count = build
            .build
            .packages
            .iter()
            .filter(|p| *p == "postgresql-dev")
            .count();
        assert_eq!(pg_count, 1, "Should not duplicate postgresql-dev");
    }
}
