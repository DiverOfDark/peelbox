use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const HASKELL: LanguageId = LanguageId::new("haskell");
const CABAL: BuildSystemId = BuildSystemId::new("cabal");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    BuildSystemMeta { slug: "cabal", display_name: "Cabal", aliases: &["cabal"] }
}

pub struct CabalFileParser;

impl ManifestParser for CabalFileParser {
    fn filenames(&self) -> &[&str] {
        // Matched by extension in pipeline.rs classify_file (like .csproj)
        &[]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Basic validation: must contain cabal-version or name field
        if !content.contains("cabal-version:") && !content.contains("name:") {
            return None;
        }

        let name = parse_cabal_field(content, "name").unwrap_or_else(|| {
            // Fall back to filename stem
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("app")
                .to_string()
        });

        let version = parse_cabal_field(content, "version");

        // Check if there's an executable section (application vs library)
        let has_executable = content.contains("executable ");
        let has_library = content.contains("library") && !has_executable;
        let is_application = has_executable || !has_library;

        // Parse build-depends
        let dependencies = parse_cabal_build_depends(content);

        Some(Manifest {
            path: path.to_path_buf(),
            language: HASKELL,
            build_system: CABAL,
            runtime: NATIVE,
            package: Some(Package {
                name: name.clone(),
                version,
                is_application,
            }),
            workspace: None,
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
                    "curl -sSL https://get-ghcup.haskell.org | BOOTSTRAP_HASKELL_NONINTERACTIVE=1 BOOTSTRAP_HASKELL_INSTALL_NO_STACK=1 bash".into(),
                    "export PATH=\"$HOME/.ghcup/bin:$PATH\"".into(),
                    "cabal update".into(),
                    "cabal build".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec![".cabal".into(), "dist-newstyle".into()],
                artifacts: if is_application {
                    vec![(
                        format!("dist-newstyle/build/**/build/{name}/{name}"),
                        format!("/app/{name}"),
                    )]
                } else {
                    vec![]
                },
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "glibc".into(),
                    "gmp".into(),
                    "zlib".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: if is_application {
                    Some(format!("/app/{name}"))
                } else {
                    None
                },
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

/// Parse a top-level field from a .cabal file (e.g., "name:", "version:").
fn parse_cabal_field(content: &str, field: &str) -> Option<String> {
    let pattern = format!("{}:", field);
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&pattern) {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Parse build-depends from a .cabal file.
/// Handles both inline and multi-line formats.
fn parse_cabal_build_depends(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut in_build_depends = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("build-depends:") {
            in_build_depends = true;
            // Parse deps on the same line after the colon
            let rest = trimmed.strip_prefix("build-depends:").unwrap_or("").trim();
            if !rest.is_empty() {
                parse_cabal_dep_list(rest, &mut deps);
            }
            continue;
        }

        if in_build_depends {
            // Continuation lines start with whitespace or comma
            // A new field (e.g., "default-language:") ends the build-depends block
            if trimmed.is_empty()
                || (!line.starts_with(' ') && !line.starts_with('\t') && !trimmed.starts_with(','))
            {
                in_build_depends = false;
                continue;
            }
            // If the indented line contains a colon and looks like a field, stop
            if trimmed.contains(':') && !trimmed.starts_with(',') {
                // Check if it looks like a cabal field (word followed by colon)
                if let Some(before_colon) = trimmed.split(':').next() {
                    let is_field = !before_colon.is_empty()
                        && before_colon
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '-');
                    if is_field {
                        in_build_depends = false;
                        continue;
                    }
                }
            }
            parse_cabal_dep_list(trimmed, &mut deps);
        }
    }

    deps
}

/// Parse a comma-separated list of cabal dependencies.
/// Format: "base >=4.14 && <5, warp >=3.3, text"
fn parse_cabal_dep_list(text: &str, deps: &mut Vec<Dependency>) {
    for part in text.split(',') {
        let part = part.trim().trim_start_matches(',').trim();
        if part.is_empty() {
            continue;
        }

        // Split on first space to separate name from version constraint
        let mut parts = part.splitn(2, char::is_whitespace);
        let name = match parts.next() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        let version = parts.next().map(|v| v.trim().to_string());

        deps.push(Dependency {
            name,
            version,
            scope: DepScope::Runtime,
            is_internal: false,
        });
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(CabalFileParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_parse_basic_cabal_file() {
        let parser = CabalFileParser;
        let content = r#"cabal-version: 2.4
name:          my-app
version:       0.1.0.0

executable my-app
  main-is:       Main.hs
  hs-source-dirs: app
  build-depends:
    base >=4.14 && <5,
    warp >=3.3
  default-language: Haskell2010
"#;

        let manifest = parser.parse(Path::new("my-app.cabal"), content).unwrap();
        assert_eq!(manifest.language, HASKELL);
        assert_eq!(manifest.build_system, CABAL);
        assert_eq!(manifest.runtime, NATIVE);
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-app");
        assert_eq!(pkg.version, Some("0.1.0.0".into()));
        assert!(pkg.is_application);
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].name, "base");
        assert_eq!(manifest.dependencies[1].name, "warp");
    }

    #[test]
    fn test_parse_cabal_library() {
        let parser = CabalFileParser;
        let content = r#"cabal-version: 2.4
name:          my-lib
version:       1.0.0

library
  exposed-modules: MyLib
  build-depends: base >=4.14 && <5
  default-language: Haskell2010
"#;

        let manifest = parser.parse(Path::new("my-lib.cabal"), content).unwrap();
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my-lib");
        assert!(!pkg.is_application);
        assert!(manifest.runtime_config.entrypoint.is_none());
    }

    #[test]
    fn test_parse_cabal_deps_multiline() {
        let content = r#"cabal-version: 2.4
name: test

executable test
  build-depends:
    base >=4.14 && <5,
    warp >=3.3,
    aeson >=2.0,
    text
  default-language: Haskell2010
"#;
        let deps = parse_cabal_build_depends(content);
        let names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["base", "warp", "aeson", "text"]);
    }

    #[test]
    fn test_parse_cabal_deps_single_line() {
        let content = r#"cabal-version: 2.4
name: test

executable test
  build-depends: base >=4.14 && <5, warp >=3.3
"#;
        let deps = parse_cabal_build_depends(content);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "base");
        assert_eq!(deps[1].name, "warp");
    }

    #[test]
    fn test_rejects_non_cabal_content() {
        let parser = CabalFileParser;
        let result = parser.parse(Path::new("something.cabal"), "Just some text file");
        assert!(result.is_none());
    }

    #[test]
    fn test_fallback_name_from_filename() {
        let parser = CabalFileParser;
        let content = r#"cabal-version: 2.4
version: 0.1.0

executable app
  main-is: Main.hs
  build-depends: base
"#;

        let manifest = parser
            .parse(Path::new("fallback-app.cabal"), content)
            .unwrap();
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "fallback-app");
    }

    #[test]
    fn test_parse_cabal_field() {
        let content = "cabal-version: 2.4\nname:          my-app\nversion: 0.1.0";
        assert_eq!(parse_cabal_field(content, "name"), Some("my-app".into()));
        assert_eq!(parse_cabal_field(content, "version"), Some("0.1.0".into()));
        assert_eq!(
            parse_cabal_field(content, "cabal-version"),
            Some("2.4".into())
        );
        assert_eq!(parse_cabal_field(content, "nonexistent"), None);
    }
}
