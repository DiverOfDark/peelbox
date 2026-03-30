use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const CRYSTAL: LanguageId = LanguageId::new("crystal");
const SHARDS: BuildSystemId = BuildSystemId::new("shards");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    LanguageMeta { slug: "crystal", display_name: "Crystal", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "shards", display_name: "Shards", aliases: &["shards"] }
}

pub struct ShardYmlParser;

impl ManifestParser for ShardYmlParser {
    fn filenames(&self) -> &[&str] {
        &["shard.yml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Manual line-based YAML parsing to avoid adding extra deps
        let name = extract_yaml_value(content, "name")?;
        let version = extract_yaml_value(content, "version");

        // Extract target name from the targets section
        let target_name = extract_first_target(content).unwrap_or_else(|| name.clone());

        let dependencies = parse_shard_deps(content);

        // Extract crystal version from the `crystal:` key, default to 1.14.0
        let crystal_version =
            extract_yaml_value(content, "crystal").unwrap_or_else(|| "1.14.0".to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: CRYSTAL,
            build_system: SHARDS,
            runtime: NATIVE,
            package: Some(Package {
                name: name.clone(),
                version,
                is_application: true,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    "build-base".into(),
                    "curl".into(),
                    "glibc-dev".into(),
                    "libevent-dev".into(),
                    "libxml2-dev".into(),
                    "yaml-dev".into(),
                    "openssl-dev".into(),
                    "pcre-dev".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    format!(
                        "curl -fsSL https://github.com/crystal-lang/crystal/releases/download/{v}/crystal-{v}-1-linux-x86_64.tar.gz | tar xz -C /usr/local --strip-components=1",
                        v = crystal_version
                    ),
                    "shards install".into(),
                    "shards build --release".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["lib".into()],
                artifacts: vec![
                    (format!("bin/{}", target_name), format!("/app/bin/{}", target_name)),
                ],
                setup_commands: vec![],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "libevent".into(), "pcre".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("./bin/{}", target_name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

/// Extract a top-level YAML scalar value by key.
fn extract_yaml_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&format!("{}:", key)) {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Extract the first target name from a `targets:` section.
///
/// Looks for:
/// ```yaml
/// targets:
///   target_name:
///     main: src/target_name.cr
/// ```
fn extract_first_target(content: &str) -> Option<String> {
    let mut in_targets = false;
    for line in content.lines() {
        if line.trim() == "targets:" {
            in_targets = true;
            continue;
        }
        if in_targets {
            // A line indented by exactly 2 spaces with a trailing colon is a target name
            if line.starts_with("  ") && !line.starts_with("    ") {
                let trimmed = line.trim();
                if let Some(name) = trimmed.strip_suffix(':') {
                    return Some(name.to_string());
                }
            }
            // If we hit a non-indented line (new top-level key), stop
            if !line.starts_with(' ') && !line.is_empty() {
                break;
            }
        }
    }
    None
}

/// Parse dependencies from the `dependencies:` section of shard.yml.
fn parse_shard_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut in_deps = false;

    for line in content.lines() {
        if line.trim() == "dependencies:" {
            in_deps = true;
            continue;
        }
        if line.trim() == "development_dependencies:" {
            // Switch to dev deps section
            in_deps = false;
            // We could parse dev deps too, but keep it simple
            continue;
        }
        if in_deps {
            // A dependency name is indented by 2 spaces and ends with colon
            if line.starts_with("  ") && !line.starts_with("    ") {
                let trimmed = line.trim();
                if let Some(name) = trimmed.strip_suffix(':') {
                    deps.push(Dependency {
                        name: name.to_string(),
                        version: None,
                        scope: DepScope::Runtime,
                        is_internal: false,
                    });
                }
            }
            // If we hit a non-indented line (new top-level key), stop
            if !line.starts_with(' ') && !line.is_empty() && line.trim() != "dependencies:" {
                in_deps = false;
            }
        }
    }

    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(ShardYmlParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_shard_yml_with_targets() {
        let parser = ShardYmlParser;
        let content = r#"name: crystal
version: 0.1.0
targets:
  crystal:
    main: src/crystal.cr
crystal: 1.4.1
"#;
        let manifest = parser.parse(Path::new("shard.yml"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::new("crystal"));
        assert_eq!(manifest.build_system, BuildSystemId::new("shards"));
        assert_eq!(manifest.runtime, RuntimeId::new("native"));
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "crystal");
        assert_eq!(pkg.version.as_deref(), Some("0.1.0"));
        assert!(pkg.is_application);
        assert_eq!(
            manifest.runtime_config.entrypoint.as_deref(),
            Some("./bin/crystal")
        );
        assert_eq!(manifest.runtime_config.ports, vec![8080]);
    }

    #[test]
    fn test_shard_yml_no_targets() {
        let parser = ShardYmlParser;
        let content = r#"name: my-app
version: 1.0.0
"#;
        let manifest = parser.parse(Path::new("shard.yml"), content).unwrap();
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        // Falls back to project name
        assert_eq!(
            manifest.runtime_config.entrypoint.as_deref(),
            Some("./bin/my-app")
        );
    }

    #[test]
    fn test_shard_yml_with_deps() {
        let parser = ShardYmlParser;
        let content = r#"name: web-app
version: 0.2.0
dependencies:
  kemal:
    github: kemalcr/kemal
  pg:
    github: will/crystal-pg
"#;
        let manifest = parser.parse(Path::new("shard.yml"), content).unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].name, "kemal");
        assert_eq!(manifest.dependencies[1].name, "pg");
    }

    #[test]
    fn test_extract_first_target() {
        let content = r#"targets:
  mybin:
    main: src/main.cr
"#;
        assert_eq!(extract_first_target(content), Some("mybin".to_string()));
    }

    #[test]
    fn test_extract_yaml_value() {
        assert_eq!(
            extract_yaml_value("name: hello", "name"),
            Some("hello".to_string())
        );
        assert_eq!(
            extract_yaml_value("version: 1.0.0", "version"),
            Some("1.0.0".to_string())
        );
    }
}
