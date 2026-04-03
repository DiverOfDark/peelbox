use crate::helpers::btree;
use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const UV: BuildSystemId = BuildSystemId::new("uv");

const PYTHON: LanguageId = LanguageId::new("python");
const PYTHON_RT: RuntimeId = RuntimeId::new("python");

/// Detects UV projects by presence of uv.lock.
/// Reads the sibling pyproject.toml for project info but overrides build system to UV.
pub struct UvLockParser;

impl ManifestParser for UvLockParser {
    fn filenames(&self) -> &[&str] {
        &["uv.lock"]
    }

    fn parse(&self, path: &Path, _content: &str) -> Option<Manifest> {
        // Read the sibling pyproject.toml
        let dir = path.parent()?;
        let pyproject_path = dir.join("pyproject.toml");
        let abs_pyproject = if pyproject_path.is_absolute() {
            pyproject_path
        } else {
            // During pipeline, the path is relative; we need an absolute path.
            // Return None for relative paths (same pattern as YarnLockParser).
            return None;
        };
        let pyproject_content = std::fs::read_to_string(&abs_pyproject).ok()?;
        let toml_val: toml::Value = toml::from_str(&pyproject_content).ok()?;

        // If [tool.uv] is already present, the PyProjectTomlParser will handle it
        // with full UV support. This parser only kicks in when uv.lock exists
        // alongside a plain pyproject.toml (no [tool.uv] section).
        if toml_val.get("tool").and_then(|t| t.get("uv")).is_some() {
            return None;
        }

        let project = toml_val.get("project");
        let name = project
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let version = project
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse dependencies from the sibling pyproject.toml so that
        // framework detection (e.g., FastAPI, Flask) works even when the
        // pyproject has no [tool.uv] section.
        let dependencies = crate::parsers::manifest::pyproject_toml::parse_pyproject_deps_public(
            &toml_val, false,
        );

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
                    "pip install --user uv".into(),
                    "/root/.local/bin/uv sync --frozen --no-dev".into(),
                ],
                member_transform: None,
                env: BTreeMap::from([("UV_CACHE_DIR".into(), "/root/.cache/uv".into())]),
                cache_dirs: vec!["/root/.cache/pip/".into(), "/root/.cache/uv/".into()],
                artifacts: vec![(".".into(), "/app".into())],
                setup_commands: vec![],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "python".into(),
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: btree(&[
                    ("PATH", "/app/.venv/bin:/usr/local/bin:/usr/bin:/bin"),
                    ("VIRTUAL_ENV", "/app/.venv"),
                ]),
                entrypoint: {
                    // Prefer [project.scripts] entrypoint from the companion pyproject.toml
                    let script_entrypoint = project
                        .and_then(|p| p.get("scripts"))
                        .and_then(|s| s.as_table())
                        .and_then(|t| t.keys().next())
                        .map(|k| k.to_string());
                    script_entrypoint.or_else(|| {
                        name.as_ref()
                            .map(|n| format!("python -m {}", n.replace('-', "_")))
                    })
                },
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(UvLockParser))
}

// ── Build System Profile ────────────────────────────────────────────────────
// Note: The UV build system profile is defined in pyproject_toml.rs since it
// handles both the [tool.uv] case and the uv.lock fallback case. The
// merge_priority here ensures uv.lock takes precedence when merging with
// a plain pyproject.toml manifest in the same directory.

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        merge_priority: true,
        use_package_name_for_root: false,
        preferred_framework_env_keys: &["VIRTUAL_ENV"],
        ..BuildSystemConfig::new(UV)
    })
}
