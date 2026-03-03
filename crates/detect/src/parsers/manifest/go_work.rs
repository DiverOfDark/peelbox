use super::go_mod::{is_go_version_below_wolfi_min, legacy_go_sdk_setup};
use crate::helpers::btree;
use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const GO: LanguageId = LanguageId::new("go");
const GOMOD: BuildSystemId = BuildSystemId::new("go-mod");
const NATIVE: RuntimeId = RuntimeId::new("native");

pub struct GoWorkParser;

impl ManifestParser for GoWorkParser {
    fn filenames(&self) -> &[&str] {
        &["go.work"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let members = parse_go_work_use(content);
        if members.is_empty() {
            return None;
        }

        // Extract Go version from go.work (e.g., "go 1.21")
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

        let mut env_pairs: Vec<(&str, &str)> = Vec::new();
        env_pairs.extend_from_slice(&extra_env);

        Some(Manifest {
            path: path.to_path_buf(),
            language: GO,
            build_system: GOMOD,
            runtime: NATIVE,
            package: None,
            workspace: Some(Workspace {
                members,
                orchestrator: None,
            }),
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: build_packages,
                commands: sdk_install_commands,
                member_transform: None,
                env: btree(&env_pairs),
                cache_dirs: vec![".cache/go-build".into(), ".cache/go-mod".into()],
                artifacts: vec![],
            },
            runtime_config: RuntimeSpec::default(),
        })
    }
}

/// Parse the `use (...)` block from a go.work file.
/// Supports both multi-line block syntax and single-line `use ./path` syntax.
fn parse_go_work_use(content: &str) -> Vec<String> {
    let mut members = Vec::new();
    let mut in_use_block = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Multi-line use block: `use (`
        if trimmed == "use (" {
            in_use_block = true;
            continue;
        }

        if in_use_block {
            if trimmed == ")" {
                in_use_block = false;
                continue;
            }
            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            let member = trimmed.trim_start_matches("./");
            if !member.is_empty() {
                members.push(member.to_string());
            }
            continue;
        }

        // Single-line use: `use ./path`
        if trimmed.starts_with("use ") && !trimmed.ends_with("(") {
            let rest = trimmed.strip_prefix("use ").unwrap_or("").trim();
            let member = rest.trim_start_matches("./");
            if !member.is_empty() {
                members.push(member.to_string());
            }
        }
    }

    members
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(GoWorkParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_go_work_parser_multiline() {
        let parser = GoWorkParser;
        let content = r#"go 1.21

use (
    ./cmd/api
    ./pkg/shared
)
"#;
        let manifest = parser.parse(Path::new("go.work"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::new("go"));
        assert_eq!(manifest.build_system, BuildSystemId::new("go-mod"));
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["cmd/api", "pkg/shared"]);
        assert!(manifest.package.is_none());
        assert!(manifest.build.packages.contains(&"go-1.21".to_string()));
    }

    #[test]
    fn test_go_work_parser_single_line() {
        let parser = GoWorkParser;
        let content = "go 1.22\n\nuse ./services/auth\n";
        let manifest = parser.parse(Path::new("go.work"), content).unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["services/auth"]);
    }

    #[test]
    fn test_go_work_parser_no_version() {
        let parser = GoWorkParser;
        let content = "use (\n    ./app\n)\n";
        let manifest = parser.parse(Path::new("go.work"), content).unwrap();
        assert!(manifest.build.packages.contains(&"go".to_string()));
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["app"]);
    }

    #[test]
    fn test_go_work_parser_empty_use() {
        let parser = GoWorkParser;
        let content = "go 1.21\n";
        let result = parser.parse(Path::new("go.work"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_go_work_parser_comments_and_blank_lines() {
        let parser = GoWorkParser;
        let content = r#"go 1.21

use (
    // this is a comment
    ./cmd/api

    ./pkg/shared
    // another comment
)
"#;
        let manifest = parser.parse(Path::new("go.work"), content).unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["cmd/api", "pkg/shared"]);
    }

    #[test]
    fn test_go_work_parser_no_dot_slash_prefix() {
        let parser = GoWorkParser;
        let content = "go 1.21\n\nuse (\n    cmd/api\n    pkg/shared\n)\n";
        let manifest = parser.parse(Path::new("go.work"), content).unwrap();
        let ws = manifest.workspace.unwrap();
        assert_eq!(ws.members, vec!["cmd/api", "pkg/shared"]);
    }
}
