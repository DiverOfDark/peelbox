use crate::helpers::btree;
use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const GO: LanguageId = LanguageId::new("go");
const GOMOD: BuildSystemId = BuildSystemId::new("go-mod");
const NATIVE: RuntimeId = RuntimeId::new("native");

/// Return the Wolfi package name for a Go version string.
/// Old versions (below Wolfi availability) use the generic "go" package,
/// which Wolfi resolution upgrades to the latest available version.
/// Go is backward-compatible, so building old code with a newer compiler is safe.
pub(crate) fn go_wolfi_package(version: Option<&str>) -> String {
    version
        .map(|v| format!("go-{}", v))
        .unwrap_or_else(|| "go".into())
}

inventory::submit! {
    LanguageMeta { slug: "go", display_name: "Go", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "go-mod", display_name: "go mod", aliases: &["go-mod"] }
}

/// Check if any `.go` file in the given directory (or its `cmd/` subdirectories)
/// declares `package main`.
/// This determines whether the Go module is an executable (application) or a library.
///
/// Returns `Some(cmd_subdir_name)` if found in `cmd/X/`, or `Some("")` if found
/// in the root directory itself, or `None` if not found.
fn find_go_main_package(dir: &Path) -> Option<String> {
    // First check the root directory itself
    if dir_has_package_main(dir) {
        return Some(String::new());
    }

    // Then check cmd/ subdirectories (common Go project layout)
    let cmd_dir = dir.join("cmd");
    if let Ok(entries) = std::fs::read_dir(&cmd_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && dir_has_package_main(&path) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    return Some(name.to_string());
                }
            }
        }
    }

    None
}

/// Check if any `.go` file in the given directory declares `package main`.
fn dir_has_package_main(dir: &Path) -> bool {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("go") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed == "package main" {
                        return true;
                    }
                    // Stop after the package declaration line
                    if trimmed.starts_with("package ") {
                        break;
                    }
                }
            }
        }
    }
    false
}

pub struct GoModParser;

impl ManifestParser for GoModParser {
    fn filenames(&self) -> &[&str] {
        &["go.mod"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let module_name = content
            .lines()
            .find(|l| l.starts_with("module "))
            .map(|l| l.trim_start_matches("module ").trim().to_string())?;

        let short_name = module_name.rsplit('/').next().unwrap_or(&module_name);

        // Extract Go version from go.mod (e.g., "go 1.21")
        let go_version = content
            .lines()
            .find(|l| l.starts_with("go "))
            .and_then(|l| {
                let ver = l.trim_start_matches("go ").trim();
                if ver.is_empty() {
                    None
                } else {
                    Some(ver.to_string())
                }
            });

        // Always use Wolfi packages. Go is backward-compatible, so a newer
        // compiler can build code written for an older version. For old versions
        // not in Wolfi, the generic "go" name is resolved to the latest available.
        let go_pkg = go_wolfi_package(go_version.as_deref());
        let build_packages = vec![go_pkg, "git".into(), "ca-certificates".into()];

        let dependencies = parse_go_deps(content);

        // Determine if this module is an executable or a library
        let dir = path.parent().unwrap_or(Path::new("."));
        let main_location = find_go_main_package(dir);
        let is_application = main_location.is_some();

        let (commands, member_transform, artifacts, entrypoint, ports) =
            if let Some(ref cmd_subdir) = main_location {
                if cmd_subdir.is_empty() {
                    // main.go is in the root directory
                    (
                        vec![
                            "go mod download".into(),
                            "mkdir -p bin".into(),
                            format!("go build -o bin/{} .", short_name),
                        ],
                        Some(MemberBuildTransform {
                            member_commands: vec![
                                "mkdir -p bin".into(),
                                format!("go build -o bin/{} ./{{module}}", short_name),
                            ],
                            member_artifacts: None,
                        }),
                        vec![(
                            format!("bin/{}", short_name),
                            format!("/app/{}", short_name),
                        )],
                        Some(format!("/app/{}", short_name)),
                        vec![8080],
                    )
                } else {
                    // main.go is in cmd/<subdir>/
                    let binary_name = cmd_subdir;
                    (
                        vec![
                            "go mod download".into(),
                            "mkdir -p bin".into(),
                            format!("go build -o bin/{} ./cmd/{}", binary_name, binary_name),
                        ],
                        Some(MemberBuildTransform {
                            member_commands: vec![
                                "mkdir -p bin".into(),
                                format!(
                                    "go build -o bin/{} ./{{module}}/cmd/{}",
                                    binary_name, binary_name
                                ),
                            ],
                            member_artifacts: None,
                        }),
                        vec![(
                            format!("bin/{}", binary_name),
                            format!("/app/{}", binary_name),
                        )],
                        Some(format!("/app/{}", binary_name)),
                        vec![8080],
                    )
                }
            } else {
                (vec![], None, vec![], None, vec![])
            };

        // Build environment
        let env_pairs: Vec<(&str, &str)> = vec![
            ("CGO_ENABLED", "0"),
            ("GOCACHE", "/app/.cache/go-build"),
            ("GOMODCACHE", "/app/.cache/go-mod"),
            ("GOSUMDB", "off"),
        ];

        Some(Manifest {
            path: path.to_path_buf(),
            language: GO,
            build_system: GOMOD,
            runtime: NATIVE,
            package: Some(Package {
                name: short_name.to_string(),
                version: None,
                is_application,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: build_packages,
                commands,
                member_transform,
                env: btree(&env_pairs),
                cache_dirs: vec![".cache/go-build".into(), ".cache/go-mod".into()],
                artifacts,
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint,
                workdir: Some("/app".into()),
                ports,
                health_endpoint: None,
            },
        })
    }
}

fn parse_go_deps(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut in_require = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "require (" {
            in_require = true;
            continue;
        }
        if trimmed == ")" {
            in_require = false;
            continue;
        }
        // Handle single-line require
        if trimmed.starts_with("require ") && !trimmed.ends_with("(") {
            let rest = trimmed.strip_prefix("require ").unwrap_or("");
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 2 {
                deps.push(Dependency {
                    name: parts[0].to_string(),
                    version: Some(parts[1].to_string()),
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
            continue;
        }
        if in_require && !trimmed.starts_with("//") {
            let clean = trimmed.split("//").next().unwrap_or(trimmed).trim();
            let parts: Vec<&str> = clean.split_whitespace().collect();
            if parts.len() >= 2 {
                deps.push(Dependency {
                    name: parts[0].to_string(),
                    version: Some(parts[1].to_string()),
                    scope: DepScope::Runtime,
                    is_internal: false,
                });
            }
        }
    }
    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(GoModParser))
}

inventory::submit! {
    crate::source_scanning::SourceScanEntry {
        languages: &["Go"],
        extensions: &["go"],
        port_patterns: &[
            r"ListenAndServe\([^)]*:(\d{4,5})",
            r#"addr\s*=\s*"[^:]*:(\d{4,5})""#,
        ],
        health_patterns: &[r#"\.(?:GET|Handle(?:Func)?)\(['"]([/\w\-]*health[/\w\-]*)['"]"#],
        env_var_patterns: &[r#"os\.Getenv\(["']([A-Z_][A-Z0-9_]*)"#],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_go_wolfi_package() {
        assert_eq!(go_wolfi_package(Some("1.21")), "go-1.21");
        assert_eq!(go_wolfi_package(Some("1.23.4")), "go-1.23.4");
        assert_eq!(go_wolfi_package(None), "go");
        // Old versions still get a versioned package name; Wolfi resolution
        // upgrades them to the latest available version.
        assert_eq!(go_wolfi_package(Some("1.18")), "go-1.18");
    }

    #[test]
    fn test_go_mod_parser_modern_version() {
        let parser = GoModParser;
        let content = "module example.com/app\n\ngo 1.21\n";
        let manifest = parser.parse(Path::new("go.mod"), content).unwrap();

        // Should use Wolfi package
        assert!(manifest.build.packages.contains(&"go-1.21".to_string()));
        assert!(!manifest.build.packages.contains(&"curl".to_string()));
        assert!(!manifest.build.packages.contains(&"build-base".to_string()));
        // Should NOT have SDK install command
        assert!(!manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("go.dev/dl")));
        // Should NOT have GOROOT env
        assert!(!manifest.build.env.contains_key("GOROOT"));
    }

    #[test]
    fn test_go_mod_parser_old_version() {
        let parser = GoModParser;
        let content = "module example.com/app\n\ngo 1.18\n";
        let manifest = parser.parse(Path::new("go.mod"), content).unwrap();

        // Old versions now use Wolfi packages too (Go is backward-compatible).
        // Wolfi resolution upgrades to latest available version.
        assert!(manifest.build.packages.contains(&"go-1.18".to_string()));
        assert!(!manifest.build.packages.contains(&"curl".to_string()));
        assert!(!manifest.build.packages.contains(&"build-base".to_string()));
        // Should NOT have GOROOT env
        assert!(!manifest.build.env.contains_key("GOROOT"));
        // Standard Go env should still be present
        assert_eq!(manifest.build.env.get("CGO_ENABLED").unwrap(), "0");
    }

    #[test]
    fn test_go_mod_parser_old_version_with_patch() {
        let parser = GoModParser;
        let content = "module example.com/app\n\ngo 1.17.5\n";
        let manifest = parser.parse(Path::new("go.mod"), content).unwrap();

        // Should use Wolfi package (not legacy SDK download)
        assert!(manifest.build.packages.contains(&"go-1.17.5".to_string()));
        assert!(!manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("go.dev/dl")));
    }
}
