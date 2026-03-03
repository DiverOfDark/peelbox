use crate::helpers::btree;
use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const GO: LanguageId = LanguageId::new("go");
const GOMOD: BuildSystemId = BuildSystemId::new("go-mod");
const NATIVE: RuntimeId = RuntimeId::new("native");

/// Minimum Go minor version available as a Wolfi package (go-1.20, go-1.21, etc.).
/// Go versions older than 1.{MIN_WOLFI_GO_MINOR} must be installed from source via go.dev.
pub(crate) const MIN_WOLFI_GO_MINOR: u32 = 20;

/// Check whether a Go version string (e.g. "1.18", "1.21.3") is below the
/// minimum version available in Wolfi. Returns `true` when the version is old
/// and must be fetched from go.dev instead.
pub(crate) fn is_go_version_below_wolfi_min(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    let major: u32 = match parts[0].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let minor: u32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => return false,
    };
    // Go 2.x+ would never be below the threshold
    if major != 1 {
        return false;
    }
    minor < MIN_WOLFI_GO_MINOR
}

/// Build the packages, extra commands, and extra env vars needed when the
/// requested Go version is too old for Wolfi.  The SDK is downloaded from
/// go.dev and unpacked into `/usr/local/go`.
pub(crate) fn legacy_go_sdk_setup(
    version: &str,
) -> (Vec<String>, Vec<String>, Vec<(&'static str, &'static str)>) {
    let install_cmd = format!(
        "curl -fsSL https://go.dev/dl/go{}.linux-amd64.tar.gz | tar -C /usr/local -xzf -",
        version
    );
    let packages = vec![
        "build-base".into(),
        "curl".into(),
        "git".into(),
        "ca-certificates".into(),
    ];
    let extra_commands = vec![install_cmd];
    let extra_env = vec![
        ("GOROOT", "/usr/local/go"),
        ("PATH", "/usr/local/go/bin:${PATH}"),
    ];
    (packages, extra_commands, extra_env)
}

inventory::submit! {
    LanguageMeta { slug: "go", display_name: "Go", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "go-mod", display_name: "go mod", aliases: &["go-mod"] }
}

/// Check if any `.go` file in the given directory declares `package main`.
/// This determines whether the Go module is an executable (application) or a library.
fn has_go_main_package(dir: &Path) -> bool {
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

        // Determine whether the Go version is available in Wolfi or needs
        // a manual SDK download from go.dev.
        let needs_legacy_sdk = go_version
            .as_ref()
            .is_some_and(|v| is_go_version_below_wolfi_min(v));

        let (build_packages, sdk_install_commands, extra_env) = if needs_legacy_sdk {
            let version = go_version.as_ref().unwrap();
            legacy_go_sdk_setup(version)
        } else {
            let go_pkg = go_version
                .as_ref()
                .map(|v| format!("go-{}", v))
                .unwrap_or_else(|| "go".into());
            (
                vec![go_pkg, "git".into(), "ca-certificates".into()],
                Vec::new(),
                Vec::new(),
            )
        };

        let dependencies = parse_go_deps(content);

        // Determine if this module is an executable or a library
        let dir = path.parent().unwrap_or(Path::new("."));
        let is_application = has_go_main_package(dir);

        let (mut commands, member_transform, artifacts, entrypoint, ports) = if is_application {
            (
                vec![
                    "go mod download".into(),
                    format!("go build -o {} .", short_name),
                ],
                Some(MemberBuildTransform {
                    member_commands: vec![format!("go build -o {} ./{{module}}", short_name)],
                    member_artifacts: None,
                }),
                vec![(short_name.to_string(), format!("/app/{}", short_name))],
                Some(format!("/app/{}", short_name)),
                vec![8080],
            )
        } else {
            (vec![], None, vec![], None, vec![])
        };

        // Prepend SDK install commands (if needed) before build commands
        if !sdk_install_commands.is_empty() {
            let mut all_commands = sdk_install_commands;
            all_commands.append(&mut commands);
            commands = all_commands;
        }

        // Build environment: standard Go env + any extra env for legacy SDK
        let mut env_pairs: Vec<(&str, &str)> = vec![
            ("CGO_ENABLED", "0"),
            ("GOCACHE", "/build/.cache/go-build"),
            ("GOMODCACHE", "/build/.cache/go-mod"),
            ("GOSUMDB", "off"),
        ];
        env_pairs.extend_from_slice(&extra_env);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_is_go_version_below_wolfi_min() {
        // Old versions should trigger legacy SDK download
        assert!(is_go_version_below_wolfi_min("1.17"));
        assert!(is_go_version_below_wolfi_min("1.18"));
        assert!(is_go_version_below_wolfi_min("1.19"));
        assert!(is_go_version_below_wolfi_min("1.18.10"));
        assert!(is_go_version_below_wolfi_min("1.16.5"));

        // Versions at or above threshold should use Wolfi packages
        assert!(!is_go_version_below_wolfi_min("1.20"));
        assert!(!is_go_version_below_wolfi_min("1.21"));
        assert!(!is_go_version_below_wolfi_min("1.22"));
        assert!(!is_go_version_below_wolfi_min("1.23.4"));

        // Edge cases
        assert!(!is_go_version_below_wolfi_min("2.0"));
        assert!(!is_go_version_below_wolfi_min("invalid"));
        assert!(!is_go_version_below_wolfi_min("1"));
    }

    #[test]
    fn test_legacy_go_sdk_setup() {
        let (packages, commands, env) = legacy_go_sdk_setup("1.18");

        assert!(packages.contains(&"build-base".to_string()));
        assert!(packages.contains(&"curl".to_string()));
        assert!(packages.contains(&"git".to_string()));
        assert!(packages.contains(&"ca-certificates".to_string()));
        assert!(!packages.iter().any(|p| p.starts_with("go-")));

        assert_eq!(commands.len(), 1);
        assert!(commands[0].contains("go.dev/dl/go1.18.linux-amd64.tar.gz"));
        assert!(commands[0].contains("tar -C /usr/local"));

        assert!(env
            .iter()
            .any(|(k, v)| *k == "GOROOT" && *v == "/usr/local/go"));
        assert!(env
            .iter()
            .any(|(k, v)| *k == "PATH" && v.contains("/usr/local/go/bin")));
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

        // Should NOT contain go-1.18 package
        assert!(!manifest.build.packages.iter().any(|p| p.starts_with("go-")));
        // Should contain build-base and curl
        assert!(manifest.build.packages.contains(&"build-base".to_string()));
        assert!(manifest.build.packages.contains(&"curl".to_string()));
        assert!(manifest.build.packages.contains(&"git".to_string()));
        assert!(manifest
            .build
            .packages
            .contains(&"ca-certificates".to_string()));
        // Should have GOROOT and PATH env
        assert_eq!(manifest.build.env.get("GOROOT").unwrap(), "/usr/local/go");
        assert!(manifest
            .build
            .env
            .get("PATH")
            .unwrap()
            .contains("/usr/local/go/bin"));
        // Standard Go env should still be present
        assert_eq!(manifest.build.env.get("CGO_ENABLED").unwrap(), "0");
    }

    #[test]
    fn test_go_mod_parser_old_version_with_patch() {
        let parser = GoModParser;
        let content = "module example.com/app\n\ngo 1.17.5\n";
        let manifest = parser.parse(Path::new("go.mod"), content).unwrap();

        // Should use legacy SDK with exact version
        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("go1.17.5.linux-amd64")));
        assert!(!manifest.build.packages.iter().any(|p| p.starts_with("go-")));
    }
}
