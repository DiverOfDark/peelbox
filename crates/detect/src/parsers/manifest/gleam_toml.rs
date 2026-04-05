use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const GLEAM: LanguageId = LanguageId::new("gleam");
const GLEAM_BS: BuildSystemId = BuildSystemId::new("gleam");
const ERLANG_BEAM: RuntimeId = RuntimeId::new("erlang-beam");

inventory::submit! {
    LanguageMeta { slug: "gleam", display_name: "Gleam", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "gleam", display_name: "Gleam", aliases: &["gleam"] }
}
inventory::submit! {
    RuntimeMeta { slug: "erlang-beam", display_name: "BEAM", aliases: &["gleam"] }
}

/// Last Gleam 0.x version that compiles `if erlang {}` syntax — used for old
/// projects with gleam_stdlib ~>0.28 that use conditional compilation blocks.
/// (0.31+ turns these into hard errors; 0.30 still accepts them as warnings.)
const GLEAM_LEGACY: &str = "0.30.0";

pub struct GleamTomlParser;

impl ManifestParser for GleamTomlParser {
    fn filenames(&self) -> &[&str] {
        &["gleam.toml"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let toml_val: toml::Value = toml::from_str(content).ok()?;

        let name = toml_val
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| "app".to_string());

        let version = toml_val
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from);

        let dependencies = parse_gleam_deps(&toml_val);

        // Determine which Gleam version to install.
        // If gleam.toml has `gleam = ">= 1.x"`, use latest.
        // Otherwise (old projects with no gleam constraint or pre-1.0 constraint),
        // pin to the last 0.x release for compatibility with old gleam_stdlib.
        let gleam_constraint = toml_val.get("gleam").and_then(|v| v.as_str()).unwrap_or("");
        let needs_v1 = gleam_constraint.contains("1.");

        // For Gleam 1.x+ projects, install the latest Gleam binary plus Erlang/rebar3
        // from Wolfi. For old pre-1.0 projects, pin to the last compatible Gleam version.
        let gleam_version = if needs_v1 { "1.13.0" } else { GLEAM_LEGACY };
        let gleam_install = format!(
            "mkdir -p /usr/local/bin && curl -fsSL https://github.com/gleam-lang/gleam/releases/download/v{v}/gleam-v{v}-$(uname -m)-unknown-linux-musl.tar.gz | tar -xzC /usr/local/bin/",
            v = gleam_version
        );
        let build_packages = vec![
            "erlang".into(),
            "erlang-dev".into(),
            "rebar3".into(),
            "curl".into(),
            "ca-certificates".into(),
        ];
        let build_commands = vec![gleam_install, "gleam export erlang-shipment".to_string()];

        Some(Manifest {
            path: path.to_path_buf(),
            language: GLEAM,
            build_system: GLEAM_BS,
            runtime: ERLANG_BEAM,
            package: Some(Package {
                name,
                version,
                is_application: true,
            }),
            workspace: None,
            dependencies,
            build: BuildSpec {
                packages: build_packages,
                commands: build_commands,
                member_transform: None,
                env: BTreeMap::from([(
                    "PATH".into(),
                    "/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
                )]),
                cache_dirs: vec!["build".into()],
                artifacts: vec![(
                    "build/erlang-shipment".into(),
                    "/app/build/erlang-shipment".into(),
                )],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["erlang".into(), "busybox".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some("/app/build/erlang-shipment/entrypoint.sh run".into()),
                workdir: Some("/app".into()),
                ports: vec![4000],
                health_endpoint: None,
            },
        })
    }
}

fn parse_gleam_deps(toml_val: &toml::Value) -> Vec<Dependency> {
    let mut deps = Vec::new();
    if let Some(table) = toml_val.get("dependencies").and_then(|v| v.as_table()) {
        for (name, val) in table {
            let version = match val {
                toml::Value::String(s) => Some(s.clone()),
                _ => None,
            };
            deps.push(Dependency {
                name: name.clone(),
                version,
                scope: DepScope::Runtime,
                is_internal: false,
            });
        }
    }
    if let Some(table) = toml_val.get("dev-dependencies").and_then(|v| v.as_table()) {
        for (name, val) in table {
            let version = match val {
                toml::Value::String(s) => Some(s.clone()),
                _ => None,
            };
            deps.push(Dependency {
                name: name.clone(),
                version,
                scope: DepScope::Dev,
                is_internal: false,
            });
        }
    }
    deps
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(GleamTomlParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_gleam_toml_basic() {
        let parser = GleamTomlParser;
        let content = r#"
name = "basic_gleam"
version = "0.1.0"

[dependencies]
gleam_stdlib = "~> 0.28.0"
"#;
        let manifest = parser.parse(Path::new("gleam.toml"), content).unwrap();
        assert_eq!(manifest.language, LanguageId::new("gleam"));
        assert_eq!(manifest.build_system, BuildSystemId::new("gleam"));
        assert_eq!(manifest.runtime, RuntimeId::new("erlang-beam"));
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "basic_gleam");
        assert_eq!(pkg.version.as_deref(), Some("0.1.0"));
        assert!(pkg.is_application);
        assert_eq!(manifest.dependencies.len(), 1);
        assert_eq!(manifest.dependencies[0].name, "gleam_stdlib");
        assert_eq!(
            manifest.runtime_config.entrypoint.as_deref(),
            Some("/app/build/erlang-shipment/entrypoint.sh run")
        );
        assert_eq!(manifest.runtime_config.ports, vec![4000]);
        assert!(manifest
            .build
            .commands
            .iter()
            .any(|c| c.contains("gleam export erlang-shipment")));
    }

    #[test]
    fn test_gleam_toml_no_deps() {
        let parser = GleamTomlParser;
        let content = r#"
name = "my_app"
version = "1.0.0"
"#;
        let manifest = parser.parse(Path::new("gleam.toml"), content).unwrap();
        let pkg = manifest.package.unwrap();
        assert_eq!(pkg.name, "my_app");
        assert!(manifest.dependencies.is_empty());
    }
}
