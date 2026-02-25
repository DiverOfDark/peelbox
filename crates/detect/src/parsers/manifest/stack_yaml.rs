use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const HASKELL: LanguageId = LanguageId::new("haskell");
const STACK: BuildSystemId = BuildSystemId::new("stack");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    LanguageMeta { slug: "haskell", display_name: "Haskell", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "stack", display_name: "Stack", aliases: &["stack"] }
}

pub struct StackYamlParser;

impl ManifestParser for StackYamlParser {
    fn filenames(&self) -> &[&str] {
        &["stack.yaml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Must contain resolver line to be a valid stack.yaml
        if !content.contains("resolver:") {
            return None;
        }

        let yaml: serde_yaml::Value = serde_yaml::from_str(content).ok()?;

        // Extract resolver for GHC version hints (e.g., "lts-21.25" or "nightly-2024-01-01")
        let _resolver = yaml
            .get("resolver")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Extract workspace members from packages field
        let workspace_members: Vec<String> = yaml
            .get("packages")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Determine if this is a workspace (multiple packages) or single project
        let workspace = if workspace_members.len() > 1
            || (workspace_members.len() == 1 && workspace_members[0] != ".")
        {
            Some(Workspace {
                members: workspace_members
                    .iter()
                    .filter(|m| *m != ".")
                    .cloned()
                    .collect(),
                orchestrator: None,
            })
        } else {
            None
        };

        // Try to find the project name from package.yaml or *.cabal in the same directory
        let dir = path.parent().unwrap_or(Path::new("."));
        let project_name = find_project_name(dir).unwrap_or_else(|| "app".to_string());

        // Parse extra-deps as dependencies
        let dependencies = parse_stack_extra_deps(&yaml);

        Some(Manifest {
            path: path.to_path_buf(),
            language: HASKELL,
            build_system: STACK,
            runtime: NATIVE,
            package: Some(Package {
                name: project_name.clone(),
                version: None,
                is_application: true,
            }),
            workspace,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    "build-base".into(),
                    "gcc".into(),
                    "gmp-dev".into(),
                    "zlib-dev".into(),
                    "ca-certificates".into(),
                    "curl".into(),
                    "bash".into(),
                ],
                commands: vec![
                    "curl -sSL https://get-ghcup.haskell.org | BOOTSTRAP_HASKELL_NONINTERACTIVE=1 BOOTSTRAP_HASKELL_INSTALL_STACK=1 BOOTSTRAP_HASKELL_INSTALL_HLS=0 bash".into(),
                    "export PATH=\"$HOME/.ghcup/bin:$PATH\"".into(),
                    "stack build --system-ghc --allow-different-user --copy-bins --local-bin-path .stack-bin".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".stack".into(), ".stack-work".into()],
                artifacts: vec![(
                    format!(".stack-bin/{}", project_name),
                    format!("/app/{}", project_name),
                )],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "glibc".into(),
                    "gmp".into(),
                    "zlib".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", project_name)),
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

/// Try to find the project name from package.yaml or *.cabal files in the directory.
fn find_project_name(dir: &Path) -> Option<String> {
    // Try package.yaml first
    let package_yaml = dir.join("package.yaml");
    if package_yaml.exists() {
        if let Ok(content) = std::fs::read_to_string(&package_yaml) {
            if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(name) = yaml.get("name").and_then(|v| v.as_str()) {
                    return Some(name.to_string());
                }
            }
        }
    }

    // Try *.cabal files
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "cabal" {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Some(name) = parse_cabal_name_field(&content) {
                            return Some(name);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Parse the `name:` field from a .cabal file.
fn parse_cabal_name_field(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name:") {
            let name = rest.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Parse extra-deps from stack.yaml as dependencies.
fn parse_stack_extra_deps(yaml: &serde_yaml::Value) -> Vec<Dependency> {
    let mut deps = Vec::new();

    if let Some(extra_deps) = yaml.get("extra-deps").and_then(|v| v.as_sequence()) {
        for dep in extra_deps {
            if let Some(dep_str) = dep.as_str() {
                // Format: "package-name-1.2.3" or "package-name-1.2.3@sha256:..."
                let clean = dep_str.split('@').next().unwrap_or(dep_str);
                // Split off version from the end (last segment after '-' that starts with digit)
                if let Some((name, version)) = split_package_version(clean) {
                    deps.push(Dependency {
                        name,
                        version: Some(version),
                        scope: DepScope::Runtime,
                        is_internal: false,
                    });
                }
            }
        }
    }

    deps
}

/// Split "package-name-1.2.3" into ("package-name", "1.2.3").
fn split_package_version(s: &str) -> Option<(String, String)> {
    // Find the last '-' that is followed by a digit
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).rev() {
        if bytes[i] == b'-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            return Some((s[..i].to_string(), s[i + 1..].to_string()));
        }
    }
    // No version found, treat whole thing as name
    Some((s.to_string(), String::new()))
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(StackYamlParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_parse_basic_stack_yaml() {
        let parser = StackYamlParser;
        let content = r#"
resolver: lts-21.25
packages:
  - .
extra-deps: []
"#;
        let manifest = parser.parse(Path::new("stack.yaml"), content).unwrap();
        assert_eq!(manifest.language, HASKELL);
        assert_eq!(manifest.build_system, STACK);
        assert_eq!(manifest.runtime, NATIVE);
        assert!(manifest.workspace.is_none());
        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("stack build")));
    }

    #[test]
    fn test_parse_stack_yaml_with_workspace() {
        let parser = StackYamlParser;
        let content = r#"
resolver: lts-21.25
packages:
  - .
  - lib/core
  - lib/utils
extra-deps: []
"#;
        let manifest = parser.parse(Path::new("stack.yaml"), content).unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["lib/core", "lib/utils"]);
    }

    #[test]
    fn test_parse_stack_yaml_with_extra_deps() {
        let parser = StackYamlParser;
        let content = r#"
resolver: lts-21.25
packages:
  - .
extra-deps:
  - warp-3.3.25
  - aeson-2.2.1.0
"#;
        let manifest = parser.parse(Path::new("stack.yaml"), content).unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].name, "warp");
        assert_eq!(manifest.dependencies[0].version, Some("3.3.25".into()));
        assert_eq!(manifest.dependencies[1].name, "aeson");
        assert_eq!(manifest.dependencies[1].version, Some("2.2.1.0".into()));
    }

    #[test]
    fn test_rejects_non_stack_yaml() {
        let parser = StackYamlParser;
        let content = r#"
name: some-other-yaml
version: 1.0
"#;
        let result = parser.parse(Path::new("stack.yaml"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_split_package_version() {
        assert_eq!(
            split_package_version("warp-3.3.25"),
            Some(("warp".into(), "3.3.25".into()))
        );
        assert_eq!(
            split_package_version("my-lib-1.0.0"),
            Some(("my-lib".into(), "1.0.0".into()))
        );
        assert_eq!(
            split_package_version("noversion"),
            Some(("noversion".into(), String::new()))
        );
    }
}
