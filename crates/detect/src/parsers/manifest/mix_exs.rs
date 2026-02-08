use crate::helpers::btree;
use crate::traits::ManifestParser;
use crate::types::*;
use peelbox_stack::{BuildSystemId, LanguageId, RuntimeId};
use std::path::Path;

pub struct MixExsParser;

impl ManifestParser for MixExsParser {
    fn filenames(&self) -> &[&str] {
        &["mix.exs"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("defmodule") || !content.contains("def project") {
            return None;
        }

        let app_re = regex::Regex::new(r#"app:\s*:(\w+)"#).ok()?;
        let name = app_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Elixir,
            build_system: BuildSystemId::Mix,
            runtime: RuntimeId::BEAM,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "elixir".into(),
                    "erlang".into(),
                    "erlang-dev".into(),
                    "git".into(),
                    "build-base".into(),
                    "openssl".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "mix local.hex --force && mix local.rebar --force".into(),
                    "mix deps.get".into(),
                    "mix compile".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("ELIXIR_ERL_OPTIONS", "+fnu"),
                    ("LC_ALL", "C.UTF-8"),
                    ("MIX_ENV", "prod"),
                    ("MIX_HOME", "/build/.mix"),
                ]),
                cache_dirs: vec!["deps".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "elixir".into(),
                    "erlang".into(),
                    "busybox".into(),
                    "openssl".into(),
                    "ca-certificates".into(),
                ],
                env: btree(&[
                    ("ELIXIR_ERL_OPTIONS", "+fnu"),
                    ("LC_ALL", "C.UTF-8"),
                    ("MIX_ENV", "prod"),
                    ("MIX_HOME", "/app/.mix"),
                    ("PORT", "4000"),
                ]),
                entrypoint: Some("mix run --no-halt".into()),
                workdir: Some("/app".into()),
                ports: vec![4000],
                health_endpoint: Some("/health".into()),
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(MixExsParser))
}
