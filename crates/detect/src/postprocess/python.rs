//! Python-specific post-processing: entrypoint scanning, native dependency
//! detection, Django settings, and Flask app path fixes.

use crate::helpers::extract_project_dir;

use ignore::WalkBuilder;
use peelbox_core::output::schema::UniversalBuild;
use std::path::Path;
use tracing::debug;

// ── Python entrypoint scanning ───────────────────────────────────────────

/// Common Python entrypoint filenames, ordered by priority.
pub(crate) const PYTHON_ENTRYPOINTS: &[&str] =
    &["app.py", "main.py", "server.py", "wsgi.py", "manage.py"];

/// Scan project directory for common Python entrypoint files.
/// Only overrides if current entrypoint is the fallback "python -m {name}".
pub fn scan_python_entrypoints(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    // Only override fallback entrypoints
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
pub fn fix_django_settings(repo_root: &Path, build: &mut UniversalBuild) {
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
        // Look for: os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'some.module')
        // or: os.environ.setdefault("DJANGO_SETTINGS_MODULE", "some.module")
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
/// Each entry: (python_package_pattern, build_packages, runtime_packages).
const PYTHON_NATIVE_DEPS: &[(&str, &[&str], &[&str])] = &[
    // PostgreSQL adapters (C compilation needs headers; binary variant also needs headers
    // as a fallback when no pre-built wheel is available for the target Python version)
    ("psycopg2", &["postgresql-dev"], &["libpq"]),
    ("psycopg2-binary", &["postgresql-dev"], &["libpq"]),
    // psycopg[c] or psycopg with C backend
    ("psycopg", &["postgresql-dev"], &["libpq"]),
    // MySQL adapter (needs both -dev for headers/symlinks and base for libmariadb.so.3)
    (
        "mysqlclient",
        &["mariadb-connector-c-dev", "mariadb-connector-c"],
        &["mariadb-connector-c"],
    ),
    // Cairo graphics
    ("pycairo", &["cairo-dev"], &["cairo"]),
    // PDF processing (poppler)
    ("pdf2image", &["poppler-utils"], &["poppler-utils"]),
    // Audio processing
    ("pydub", &["ffmpeg"], &["ffmpeg"]),
    // Pillow (image processing)
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
    // Cryptography
    (
        "cryptography",
        &["openssl-dev", "libffi-dev"],
        &["openssl", "libffi"],
    ),
    // lxml
    (
        "lxml",
        &["libxml2-dev", "libxslt-dev"],
        &["libxml2", "libxslt"],
    ),
    // cffi
    ("cffi", &["libffi-dev"], &["libffi"]),
];

/// Scan Python dependencies and add required system packages for native extensions.
pub fn scan_python_native_deps(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    // Collect Python dependency names from all supported manifest files
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
        // Native deps that compile C extensions need Python dev headers (Python.h).
        // Before Wolfi resolution, the package may be "python" (unversioned) or "python-3.X" (versioned).
        // Add the corresponding dev package so headers are available during build.
        if !build_pkgs_to_add.is_empty() {
            let dev_pkg = if let Some(py_pkg) = build
                .build
                .packages
                .iter()
                .find(|p| p.starts_with("python-3."))
            {
                Some(format!("{}-dev", py_pkg))
            } else if build.build.packages.iter().any(|p| p == "python") {
                // Unversioned — will be resolved by Wolfi; add unversioned dev placeholder
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
pub fn dep_matches(dep_name: &str, pattern: &str) -> bool {
    let normalized = dep_name.to_lowercase().replace('-', "_");
    let pat_normalized = pattern.to_lowercase().replace('-', "_");
    normalized == pat_normalized || normalized.starts_with(&format!("{}[", pat_normalized))
}

/// Collect Python dependency names from requirements.txt, pyproject.toml, Pipfile, and uv.lock.
pub fn collect_python_dep_names(project_dir: &Path) -> Vec<String> {
    let mut deps = Vec::new();

    // requirements.txt
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

    // pyproject.toml
    if let Ok(content) = std::fs::read_to_string(project_dir.join("pyproject.toml")) {
        if let Ok(toml_val) = toml::from_str::<toml::Value>(content.trim()) {
            // [project] dependencies
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

            // [tool.poetry.dependencies]
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

    // Pipfile
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
/// Searches the project directory for `app.py` or `main.py` and updates accordingly.
/// If no Flask app file is found, falls back to a Python entrypoint command.
pub fn fix_flask_app_path(repo_root: &Path, build: &mut UniversalBuild) {
    if build.metadata.language != "Python" {
        return;
    }

    // Only fix if FLASK_APP is set to one of the default hardcoded values
    let flask_app = match build.runtime.env.get("FLASK_APP") {
        Some(v) if v == "/app/app.py" => v.clone(),
        _ => return,
    };

    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    let workdir = &build.runtime.workdir;

    // If app.py exists at project root, the default FLASK_APP is correct
    if project_dir.join("app.py").exists() && project_dir == repo_root {
        return;
    }

    // For workspace members in subdirectories, search for app.py in the project dir
    if project_dir != repo_root {
        // Check for app.py directly in the project directory
        if project_dir.join("app.py").exists() {
            let rel_path = project_dir
                .strip_prefix(repo_root)
                .unwrap_or(project_dir.as_path());
            let new_flask_app = format!("{}/{}/app.py", workdir, rel_path.display());
            debug!(old = %flask_app, new = %new_flask_app, "Fixed FLASK_APP for workspace member");
            build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
            return;
        }

        // Search recursively for app.py in the project directory (e.g., src/api/app.py)
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

    // No app.py found anywhere — check for main.py with Flask imports
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

    // Search recursively for any Python file with Flask imports (e.g., package/__main__.py)
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
                    // If it's a __main__.py inside a package directory, use the
                    // Python module import form for FLASK_APP. File paths don't
                    // work for __main__.py because Flask resolves it to the
                    // built-in __main__ module.
                    //
                    // This only works when the directory is a proper Python
                    // package (has __init__.py) so that import resolution
                    // succeeds. Without __init__.py, fall back to running the
                    // script directly via `python path/__main__.py`.
                    if rel_path.file_name().is_some_and(|n| n == "__main__.py") {
                        if let Some(parent) = rel_path.parent() {
                            let dir_name = parent.to_string_lossy();
                            if !dir_name.is_empty() {
                                let abs_parent = project_dir.join(parent);
                                if abs_parent.join("__init__.py").exists() {
                                    // Proper package — convert to importable module name:
                                    // - Replace hyphens with underscores (PEP 503)
                                    // - Replace path separators with dots
                                    let pkg_name = dir_name
                                        .replace('-', "_")
                                        .replace(std::path::MAIN_SEPARATOR, ".");
                                    let new_flask_app = format!("{}.__main__:app", pkg_name);
                                    build.runtime.env.insert("FLASK_APP".into(), new_flask_app);
                                    return;
                                }
                                // No __init__.py — not a proper Python package.
                                // Flask's `flask run` can't import __main__.py from
                                // non-package directories, so fall back to running
                                // the script directly.
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

    // No Flask app file found at all — fall back to Python entrypoint
    // Remove Flask-specific runtime config and use a generic Python entrypoint
    debug!("No Flask app file found, falling back to Python entrypoint");
    build.runtime.env.remove("FLASK_APP");
    build.runtime.env.remove("FLASK_RUN_HOST");
    build.runtime.env.remove("FLASK_RUN_PORT");
    build.runtime.ports.clear();
    build.runtime.health = None;

    // Find a suitable Python entrypoint
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
    use peelbox_core::output::schema::{BuildMetadata, BuildStage, RuntimeStage};
    use std::collections::BTreeMap;

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
                setup_commands: vec![],
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
                setup_commands: vec![],
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
                setup_commands: vec![],
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
                .build
                .packages
                .contains(&"mariadb-connector-c".to_string()),
            "Should add mariadb-connector-c to build packages for mysqlclient (provides libmariadb.so.3)"
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
                setup_commands: vec![],
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
