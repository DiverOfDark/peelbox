use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const RUST: LanguageId = LanguageId::new("rust");
const CARGO: BuildSystemId = BuildSystemId::new("cargo");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    LanguageMeta { slug: "rust", display_name: "Rust", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "cargo", display_name: "Cargo", aliases: &["cargo"] }
}
inventory::submit! {
    RuntimeMeta { slug: "native", display_name: "Native", aliases: &["rust", "c++", "go"] }
}

pub struct CargoTomlParser;

impl ManifestParser for CargoTomlParser {
    fn filenames(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let toml_val: toml::Value = toml::from_str(content).ok()?;

        // Check if this is a binary crate (has src/main.rs, [[bin]] section, or autobins)
        let has_bin_section = toml_val
            .get("bin")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        let dir = path.parent().unwrap_or(Path::new("."));
        let has_main_rs = dir.join("src/main.rs").exists();
        let is_application = has_bin_section || has_main_rs;

        let package = toml_val.get("package").and_then(|pkg| {
            let name = pkg.get("name")?.as_str()?.to_string();
            let version = pkg
                .get("version")
                .and_then(|v| v.as_str())
                .map(String::from);
            Some(Package {
                name,
                version,
                is_application,
            })
        });

        let workspace = toml_val.get("workspace").map(|ws| Workspace {
            members: extract_toml_string_array(ws, "members"),
            orchestrator: None,
        });

        if package.is_none() && workspace.is_none() {
            return None;
        }

        let bin_name = package
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "app".to_string());

        let dependencies = parse_cargo_deps(&toml_val);

        let (artifacts, entrypoint) = if is_application {
            (
                vec![(
                    format!("target/release/{}", bin_name),
                    format!("/app/{}", bin_name),
                )],
                Some(format!("/app/{}", bin_name)),
            )
        } else {
            (vec![], None)
        };

        // Ports are NOT hardcoded here — they come from:
        // 1. Source code scanning (scan_source_ports in pipeline.rs)
        // 2. Framework detection (e.g., Actix Web detector)
        // 3. Config files (Dockerfile EXPOSE, .env, etc.)

        let member_transform = if is_application {
            Some(MemberBuildTransform {
                member_commands: vec![
                    "cargo build --release --manifest-path {module}/Cargo.toml --target-dir target"
                        .into(),
                ],
                member_artifacts: Some(vec![(
                    format!("target/release/{}", bin_name),
                    format!("/app/{}", bin_name),
                )]),
            })
        } else {
            None
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language: RUST,
            build_system: CARGO,
            runtime: NATIVE,
            package,
            workspace,
            dependencies,
            build: BuildSpec {
                packages: vec![
                    "rust".into(),
                    "build-base".into(),
                    "openssl-dev".into(),
                    "pkgconf".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["cargo build --release".into()],
                member_transform,
                env: btree(&[("CARGO_HOME", ".cargo")]),
                cache_dirs: vec![".cargo".into(), "target".into()],
                artifacts,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint,
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

fn extract_toml_string_array(val: &toml::Value, key: &str) -> Vec<String> {
    val.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_cargo_deps(toml_val: &toml::Value) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for (section, scope) in &[
        ("dependencies", DepScope::Runtime),
        ("dev-dependencies", DepScope::Dev),
        ("build-dependencies", DepScope::Build),
    ] {
        if let Some(table) = toml_val.get(section).and_then(|v| v.as_table()) {
            for (name, val) in table {
                let version = match val {
                    toml::Value::String(s) => Some(s.clone()),
                    toml::Value::Table(t) => {
                        t.get("version").and_then(|v| v.as_str()).map(String::from)
                    }
                    _ => None,
                };
                deps.push(Dependency {
                    name: name.clone(),
                    version,
                    scope: scope.clone(),
                    is_internal: false,
                });
            }
        }
    }
    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(CargoTomlParser))
}
