use crate::helpers::btree;
use crate::ids::BuildSystemId;
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const PYTHON: crate::ids::LanguageId = crate::ids::LanguageId::new("python");
const PDM: BuildSystemId = BuildSystemId::new("pdm");
const PYTHON_RT: crate::ids::RuntimeId = crate::ids::RuntimeId::new("python");

/// Detects PDM projects by presence of pdm.lock.
/// Reads the sibling pyproject.toml for project info but overrides build system to PDM.
pub struct PdmLockParser;

impl ManifestParser for PdmLockParser {
    fn filenames(&self) -> &[&str] {
        &["pdm.lock"]
    }

    fn parse(&self, path: &Path, _content: &str) -> Option<Manifest> {
        // Read the sibling pyproject.toml
        let dir = path.parent()?;
        let pyproject_path = dir.join("pyproject.toml");
        let abs_pyproject = if pyproject_path.is_absolute() {
            pyproject_path
        } else {
            return None;
        };
        let pyproject_content = std::fs::read_to_string(&abs_pyproject).ok()?;
        let toml_val: toml::Value = toml::from_str(&pyproject_content).ok()?;

        // If [tool.pdm] is already present, the PyProjectTomlParser will handle it
        if toml_val.get("tool").and_then(|t| t.get("pdm")).is_some() {
            return None;
        }

        // Also skip if this is a UV project (uv.lock takes precedence)
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

        let python_version = project
            .and_then(|p| p.get("requires-python"))
            .and_then(|v| v.as_str())
            .and_then(|s| {
                let trimmed = s.trim();
                if trimmed.starts_with("==") {
                    let re = regex::Regex::new(r"==(\d+\.\d+)").ok()?;
                    re.captures(trimmed)
                        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
                } else {
                    None
                }
            });

        let python_build_pkg = python_version
            .as_ref()
            .map(|v| format!("python-{}", v))
            .unwrap_or_else(|| "python".into());
        let python_runtime_pkg = python_build_pkg.clone();

        let dependencies =
            crate::parsers::manifest::pyproject_toml::parse_pyproject_deps_public(&toml_val, false);

        Some(Manifest {
            path: path.to_path_buf(),
            language: PYTHON,
            build_system: PDM,
            runtime: PYTHON_RT,
            package: Some(Package {
                name: name.unwrap_or_else(|| "app".to_string()),
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
                commands: vec![
                    "pip install pdm".into(),
                    "pdm install --prod --no-self".into(),
                ],
                member_transform: None,
                env: btree(&[("PDM_PYTHON", "/usr/bin/python3")]),
                cache_dirs: vec!["/root/.cache/pip/".into(), "/root/.cache/pdm/".into()],
                artifacts: vec![(".".into(), "/app".into())],
                setup_commands: vec![],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    python_runtime_pkg,
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: std::collections::BTreeMap::new(),
                entrypoint: None,
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PdmLockParser))
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        merge_priority: true,
        use_package_name_for_root: false,
        preferred_framework_env_keys: &["VIRTUAL_ENV"],
        ..BuildSystemConfig::new(PDM)
    })
}
