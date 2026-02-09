use crate::helpers::btree;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

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

        let go_pkg = go_version
            .as_ref()
            .map(|v| format!("go-{}", v))
            .unwrap_or_else(|| "go".into());

        let dependencies = parse_go_deps(content);

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Go,
            build_system: BuildSystemId::GoMod,
            runtime: RuntimeId::Native,
            package: Some(Package {
                name: short_name.to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: vec![go_pkg, "ca-certificates".into()],
                commands: vec![
                    "go mod download".into(),
                    format!("go build -o {} .", short_name),
                ],
                member_transform: None,
                env: btree(&[
                    ("CGO_ENABLED", "0"),
                    ("GOCACHE", "/build/.cache/go-build"),
                    ("GOMODCACHE", "/build/.cache/go-mod"),
                    ("GOSUMDB", "off"),
                ]),
                cache_dirs: vec![".cache/go-build".into(), ".cache/go-mod".into()],
                artifacts: vec![(short_name.to_string(), format!("/app/{}", short_name))],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", short_name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
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
