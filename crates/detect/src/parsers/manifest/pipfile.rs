use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const PYTHON: LanguageId = LanguageId::new("python");
const PIPENV: BuildSystemId = BuildSystemId::new("pipenv");
const PYTHON_RT: RuntimeId = RuntimeId::new("python");

inventory::submit! {
    BuildSystemMeta { slug: "pipenv", display_name: "Pipenv", aliases: &["pipenv"] }
}

pub struct PipfileParser;

impl ManifestParser for PipfileParser {
    fn filenames(&self) -> &[&str] {
        &["Pipfile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let toml_val: toml::Value = toml::from_str(content).ok()?;

        let mut deps = Vec::new();

        // Parse [packages] → runtime deps
        if let Some(packages) = toml_val.get("packages").and_then(|v| v.as_table()) {
            for (name, val) in packages {
                deps.push(Dependency {
                    name: name.clone(),
                    version: extract_pipfile_version(val),
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
        }

        // Parse [dev-packages] → dev deps
        if let Some(dev_packages) = toml_val.get("dev-packages").and_then(|v| v.as_table()) {
            for (name, val) in dev_packages {
                deps.push(Dependency {
                    name: name.clone(),
                    version: extract_pipfile_version(val),
                    scope: DepScope::Dev,
                    is_internal: false,
                });
            }
        }

        if deps.is_empty() {
            return None;
        }

        // Extract python_version from [requires] section
        let python_version = toml_val
            .get("requires")
            .and_then(|r| r.get("python_version"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let python_build_pkg = python_version
            .as_ref()
            .map(|v| format!("python-{}", v))
            .unwrap_or_else(|| "python".into());
        let python_runtime_pkg = python_build_pkg.clone();

        Some(Manifest {
            path: path.to_path_buf(),
            language: PYTHON,
            build_system: PIPENV,
            runtime: PYTHON_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: vec![
                    python_build_pkg,
                    "pip".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "pip install --user pipenv".into(),
                    "/root/.local/bin/pipenv lock --python $(which python3) && /root/.local/bin/pipenv requirements > /tmp/requirements.txt && pip install --user --no-cache-dir -r /tmp/requirements.txt".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["/root/.cache/pip/".into()],
                artifacts: vec![(".".into(), "/app/".into())],
                            setup_commands: vec![],
                build_image: None,
},
            runtime_config: RuntimeSpec {
                packages: vec![
                    python_runtime_pkg,
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: None,
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

/// Extract version from a Pipfile dependency value, normalizing "*" to None.
fn extract_pipfile_version(val: &toml::Value) -> Option<String> {
    match val {
        toml::Value::String(s) if s != "*" => Some(s.clone()),
        toml::Value::Table(t) => t
            .get("version")
            .and_then(|v| v.as_str())
            .filter(|s| *s != "*")
            .map(String::from),
        _ => None,
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PipfileParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_pipfile_basic() {
        let parser = PipfileParser;
        let content = r#"
[[source]]
url = "https://pypi.org/simple"
verify_ssl = true
name = "pypi"

[packages]
flask = "==3.0.0"
requests = "*"

[dev-packages]
pytest = "*"

[requires]
python_version = "3.11"
"#;
        let manifest = parser.parse(Path::new("Pipfile"), content).unwrap();
        assert_eq!(manifest.language, PYTHON);
        assert_eq!(manifest.build_system, PIPENV);
        assert_eq!(manifest.runtime, PYTHON_RT);

        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app");
        assert!(pkg.is_application);

        // 2 runtime + 1 dev
        assert_eq!(manifest.dependencies.len(), 3);
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "flask" && d.scope == DepScope::Runtime));
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "requests" && d.scope == DepScope::Runtime));
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "pytest" && d.scope == DepScope::Dev));

        // Check python version in build packages
        assert!(manifest.build.packages.contains(&"python-3.11".to_string()));
    }

    #[test]
    fn test_pipfile_no_packages_returns_none() {
        let parser = PipfileParser;
        let content = r#"
[[source]]
url = "https://pypi.org/simple"

[requires]
python_version = "3.11"
"#;
        assert!(parser.parse(Path::new("Pipfile"), content).is_none());
    }

    #[test]
    fn test_pipfile_version_extraction() {
        let parser = PipfileParser;
        let content = r#"
[packages]
flask = "==3.0.0"

[dev-packages]
pytest = "*"

[requires]
python_version = "3.12"
"#;
        let manifest = parser.parse(Path::new("Pipfile"), content).unwrap();
        assert!(manifest.build.packages.contains(&"python-3.12".to_string()));
        assert!(manifest
            .runtime_config
            .packages
            .contains(&"python-3.12".to_string()));

        // flask version should be captured (not "*")
        let flask_dep = manifest
            .dependencies
            .iter()
            .find(|d| d.name == "flask")
            .unwrap();
        assert_eq!(flask_dep.version, Some("==3.0.0".to_string()));

        // pytest has "*" which should be None
        let pytest_dep = manifest
            .dependencies
            .iter()
            .find(|d| d.name == "pytest")
            .unwrap();
        assert_eq!(pytest_dep.version, None);
    }

    #[test]
    fn test_pipfile_table_wildcard_normalized_to_none() {
        let parser = PipfileParser;
        let content = r#"
[packages]
flask = {version = "==3.0.0"}
requests = {version = "*"}
gunicorn = "*"
"#;
        let manifest = parser.parse(Path::new("Pipfile"), content).unwrap();

        let flask = manifest
            .dependencies
            .iter()
            .find(|d| d.name == "flask")
            .unwrap();
        assert_eq!(flask.version, Some("==3.0.0".to_string()));

        let requests = manifest
            .dependencies
            .iter()
            .find(|d| d.name == "requests")
            .unwrap();
        assert_eq!(
            requests.version, None,
            "Table {{version = \"*\"}} should normalize to None"
        );

        let gunicorn = manifest
            .dependencies
            .iter()
            .find(|d| d.name == "gunicorn")
            .unwrap();
        assert_eq!(
            gunicorn.version, None,
            "String \"*\" should normalize to None"
        );
    }
}
