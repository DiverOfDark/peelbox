use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const DART: LanguageId = LanguageId::new("dart");
const PUB: BuildSystemId = BuildSystemId::new("pub");
const DART_RT: RuntimeId = RuntimeId::new("dart");

inventory::submit! {
    LanguageMeta { slug: "dart", display_name: "Dart", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "pub", display_name: "Pub", aliases: &["pub", "dart"] }
}
inventory::submit! {
    RuntimeMeta { slug: "dart", display_name: "Dart", aliases: &["dart"] }
}

pub struct PubspecYamlParser;

impl ManifestParser for PubspecYamlParser {
    fn filenames(&self) -> &[&str] {
        &["pubspec.yaml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Manual line-based YAML parsing
        let name = extract_yaml_value(content, "name")?;
        let version = extract_yaml_value(content, "version");

        let dependencies = parse_pubspec_deps(content);

        // Determine the entry point: check for bin/{name}.dart, otherwise default to bin/main.dart
        let entry_source = format!("bin/{}.dart", name);
        let entry_exists = path
            .parent()
            .map(|d| d.join(&entry_source).exists())
            .unwrap_or(false);

        let (compile_source, binary_name) = if entry_exists {
            (entry_source, name.clone())
        } else {
            ("bin/main.dart".to_string(), "main".to_string())
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language: DART,
            build_system: PUB,
            runtime: DART_RT,
            package: Some(Package {
                name: name.clone(),
                version,
                is_application: true,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec!["dart".into(), "ca-certificates".into()],
                commands: vec![
                    "dart pub get".into(),
                    format!("dart compile exe {} -o bin/{}", compile_source, binary_name),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".dart_tool".into()],
                artifacts: vec![(
                    format!("bin/{}", binary_name),
                    format!("/app/bin/{}", binary_name),
                )],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("./bin/{}", binary_name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

/// Extract a top-level YAML scalar value by key (simple line-based parsing).
fn extract_yaml_value(content: &str, key: &str) -> Option<String> {
    let prefix = format!("{}:", key);
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Parse dependencies from pubspec.yaml.
fn parse_pubspec_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut in_section: Option<DepScope> = None;

    for line in content.lines() {
        // Top-level section detection
        if !line.starts_with(' ') && !line.is_empty() {
            let trimmed = line.trim();
            if trimmed == "dependencies:" {
                in_section = Some(DepScope::Runtime);
                continue;
            } else if trimmed == "dev_dependencies:" {
                in_section = Some(DepScope::Dev);
                continue;
            } else if trimmed.ends_with(':') || !trimmed.starts_with('#') {
                in_section = None;
                continue;
            }
        }

        if let Some(ref scope) = in_section {
            // Dependency lines are indented by 2 spaces and have the form:
            //   dep_name: version_constraint
            //   dep_name:
            if line.starts_with("  ") && !line.starts_with("    ") {
                let trimmed = line.trim();
                if let Some(colon_pos) = trimmed.find(':') {
                    let name = trimmed[..colon_pos].trim();
                    if !name.is_empty() && !name.starts_with('#') {
                        let version_part = trimmed[colon_pos + 1..].trim();
                        let version = if version_part.is_empty() {
                            None
                        } else {
                            Some(
                                version_part
                                    .trim_matches('"')
                                    .trim_matches('\'')
                                    .to_string(),
                            )
                        };
                        deps.push(Dependency {
                            name: name.to_string(),
                            version,
                            scope: scope.clone(),
                            is_internal: false,
                        });
                    }
                }
            }
        }
    }

    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PubspecYamlParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_pubspec_yaml_basic() {
        let parser = PubspecYamlParser;
        let content = r#"name: console_simple
description: A simple command-line application.
version: 1.0.0

environment:
  sdk: '>=2.16.1 <3.0.0'
"#;
        let manifest = parser.parse(Path::new("pubspec.yaml"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::new("dart"));
        assert_eq!(manifest.build_system, BuildSystemId::new("pub"));
        assert_eq!(manifest.runtime, RuntimeId::new("dart"));
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "console_simple");
        assert_eq!(pkg.version.as_deref(), Some("1.0.0"));
        assert!(pkg.is_application);
        // Without the bin file on disk, falls back to bin/main.dart
        assert_eq!(
            manifest.runtime_config.entrypoint.as_deref(),
            Some("./bin/main")
        );
        assert_eq!(manifest.runtime_config.ports, vec![8080]);
    }

    #[test]
    fn test_pubspec_yaml_with_deps() {
        let parser = PubspecYamlParser;
        let content = r#"name: my_server
version: 0.1.0

dependencies:
  shelf: ^1.4.0
  shelf_router: ^1.1.0

dev_dependencies:
  test: ^1.24.0
"#;
        let manifest = parser.parse(Path::new("pubspec.yaml"), content).unwrap();
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.dependencies[0].name, "shelf");
        assert_eq!(manifest.dependencies[0].scope, DepScope::Runtime);
        assert_eq!(manifest.dependencies[2].name, "test");
        assert_eq!(manifest.dependencies[2].scope, DepScope::Dev);
    }

    #[test]
    fn test_pubspec_yaml_no_version() {
        let parser = PubspecYamlParser;
        let content = r#"name: minimal_app
"#;
        let manifest = parser.parse(Path::new("pubspec.yaml"), content).unwrap();
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "minimal_app");
        assert!(pkg.version.is_none());
    }
}
