use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const PYTHON: LanguageId = LanguageId::new("python");
const CONDA: BuildSystemId = BuildSystemId::new("conda");
const PYTHON_RT: RuntimeId = RuntimeId::new("python");

inventory::submit! {
    BuildSystemMeta { slug: "conda", display_name: "Conda", aliases: &["conda", "micromamba"] }
}

pub struct EnvironmentYmlParser;

impl ManifestParser for EnvironmentYmlParser {
    fn filenames(&self) -> &[&str] {
        &["environment.yml", "environment.yaml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let yaml_val: serde_yaml::Value = serde_yaml::from_str(content).ok()?;

        // Must have dependencies to be a valid environment file
        let deps_val = yaml_val.get("dependencies")?;
        let deps_seq = deps_val.as_sequence()?;

        if deps_seq.is_empty() {
            return None;
        }

        let mut dependencies = Vec::new();
        let mut python_version: Option<String> = None;

        for item in deps_seq {
            if let Some(dep_str) = item.as_str() {
                let (name, version) = parse_conda_dep(dep_str);

                // Track python version for build/runtime packages
                if name == "python" {
                    if let Some(ref v) = version {
                        python_version = extract_major_minor(v);
                    }
                    continue;
                }

                dependencies.push(Dependency {
                    name,
                    version,
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            } else if let Some(mapping) = item.as_mapping() {
                // Handle pip sub-dependencies: - pip: [...]
                if let Some(pip_deps) = mapping
                    .get(serde_yaml::Value::String("pip".into()))
                    .and_then(|v| v.as_sequence())
                {
                    for pip_dep in pip_deps {
                        if let Some(pip_str) = pip_dep.as_str() {
                            let (name, version) = parse_pip_dep(pip_str);
                            dependencies.push(Dependency {
                                name,
                                version,
                                scope: DepScope::Runtime,
                                is_internal: false,
                            });
                        }
                    }
                }
            }
        }

        if dependencies.is_empty() {
            return None;
        }

        let python_build_pkg = python_version
            .as_ref()
            .map(|v| format!("python-{}", v))
            .unwrap_or_else(|| "python".into());
        let python_runtime_pkg = python_build_pkg.clone();

        // Build pip install arguments from parsed dependencies.
        // Conda deps use single = (e.g., numpy=1.24), pip uses == (e.g., gunicorn==21.2.0).
        // For pip install, we convert conda-style single = to == for pinning.
        let pip_packages: Vec<String> = dependencies
            .iter()
            .map(|d| match &d.version {
                Some(v) if v.starts_with('=') && !v.starts_with("==") => {
                    // Convert conda single-= to pip ==
                    format!("{}={}", d.name, v)
                }
                Some(v) => format!("{}{}", d.name, v),
                None => d.name.clone(),
            })
            .collect();

        let install_cmd = format!(
            "pip install --user --no-cache-dir {}",
            pip_packages.join(" ")
        );

        Some(Manifest {
            path: path.to_path_buf(),
            language: PYTHON,
            build_system: CONDA,
            runtime: PYTHON_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
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
                commands: vec![install_cmd],
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

/// Parse a conda dependency string like "numpy=1.24" or "python=3.11.4"
fn parse_conda_dep(dep_str: &str) -> (String, Option<String>) {
    // Conda uses single = for version pinning: numpy=1.24
    // Also supports ==, >=, etc.
    if let Some(idx) = dep_str.find('=') {
        let name = dep_str[..idx].trim().to_string();
        let version_part = dep_str[idx..].trim().to_string();
        (name, Some(version_part))
    } else {
        (dep_str.trim().to_string(), None)
    }
}

/// Parse a pip dependency string like "gunicorn==21.2.0"
fn parse_pip_dep(dep_str: &str) -> (String, Option<String>) {
    let name = dep_str
        .split(&['>', '<', '=', '~', '!', '['][..])
        .next()
        .unwrap_or(dep_str)
        .trim()
        .to_string();

    let version = dep_str
        .find(&['>', '<', '=', '~', '!'][..])
        .map(|idx| dep_str[idx..].trim().to_string());

    (name, version)
}

/// Extract major.minor from a conda version string like "=3.11" or "=3.11.4"
fn extract_major_minor(version: &str) -> Option<String> {
    let trimmed = version.trim_start_matches('=').trim();
    let re = regex::Regex::new(r"^(\d+\.\d+)").ok()?;
    re.captures(trimmed)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(EnvironmentYmlParser))
}

inventory::submit! {
    crate::registry::BuildSystemProfileEntry(|| BuildSystemConfig {
        use_package_name_for_root: false,
        ..BuildSystemConfig::new(CONDA)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_environment_yml_basic() {
        let parser = EnvironmentYmlParser;
        let content = r#"
name: myenv
channels:
  - defaults
  - conda-forge
dependencies:
  - python=3.11
  - numpy=1.24
  - flask=3.0
  - pip:
    - gunicorn==21.2.0
"#;
        let manifest = parser.parse(Path::new("environment.yml"), content).unwrap();
        assert_eq!(manifest.language, PYTHON);
        assert_eq!(manifest.build_system, CONDA);
        assert_eq!(manifest.runtime, PYTHON_RT);

        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "app");
        assert!(pkg.is_application);

        // Should have numpy, flask, gunicorn (python is excluded)
        assert_eq!(manifest.dependencies.len(), 3);
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "numpy" && d.version == Some("=1.24".into())));
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "flask" && d.version == Some("=3.0".into())));
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "gunicorn" && d.version == Some("==21.2.0".into())));

        // Check python version in build packages
        assert!(manifest.build.packages.contains(&"python-3.11".to_string()));
    }

    #[test]
    fn test_environment_yml_no_deps_returns_none() {
        let parser = EnvironmentYmlParser;
        let content = r#"
name: empty
channels:
  - defaults
dependencies: []
"#;
        assert!(parser
            .parse(Path::new("environment.yml"), content)
            .is_none());
    }

    #[test]
    fn test_environment_yml_python_only_returns_none() {
        let parser = EnvironmentYmlParser;
        let content = r#"
name: python-only
dependencies:
  - python=3.11
"#;
        assert!(parser
            .parse(Path::new("environment.yml"), content)
            .is_none());
    }

    #[test]
    fn test_environment_yml_no_python_version() {
        let parser = EnvironmentYmlParser;
        let content = r#"
name: no-python
dependencies:
  - numpy
  - pandas
"#;
        let manifest = parser.parse(Path::new("environment.yml"), content).unwrap();
        assert!(manifest.build.packages.contains(&"python".to_string()));
    }

    #[test]
    fn test_environment_yaml_extension() {
        let parser = EnvironmentYmlParser;
        assert!(parser.filenames().contains(&"environment.yaml"));
        assert!(parser.filenames().contains(&"environment.yml"));
    }

    #[test]
    fn test_environment_yml_pip_subdeps() {
        let parser = EnvironmentYmlParser;
        let content = r#"
name: with-pip
dependencies:
  - python=3.11
  - numpy
  - pip:
    - flask>=3.0
    - gunicorn==21.2.0
"#;
        let manifest = parser.parse(Path::new("environment.yml"), content).unwrap();
        assert_eq!(manifest.dependencies.len(), 3);
        assert!(manifest
            .dependencies
            .iter()
            .any(|d| d.name == "flask" && d.version == Some(">=3.0".into())));
    }

    #[test]
    fn test_parse_conda_dep() {
        assert_eq!(
            parse_conda_dep("numpy=1.24"),
            ("numpy".into(), Some("=1.24".into()))
        );
        assert_eq!(
            parse_conda_dep("python=3.11.4"),
            ("python".into(), Some("=3.11.4".into()))
        );
        assert_eq!(parse_conda_dep("pandas"), ("pandas".into(), None));
    }

    #[test]
    fn test_parse_pip_dep() {
        assert_eq!(
            parse_pip_dep("gunicorn==21.2.0"),
            ("gunicorn".into(), Some("==21.2.0".into()))
        );
        assert_eq!(
            parse_pip_dep("flask>=3.0"),
            ("flask".into(), Some(">=3.0".into()))
        );
        assert_eq!(parse_pip_dep("requests"), ("requests".into(), None));
    }

    #[test]
    fn test_extract_major_minor() {
        assert_eq!(extract_major_minor("=3.11"), Some("3.11".into()));
        assert_eq!(extract_major_minor("=3.11.4"), Some("3.11".into()));
        assert_eq!(extract_major_minor("3.11"), Some("3.11".into()));
    }
}
