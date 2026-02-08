use crate::helpers::btree;
use crate::traits::ManifestParser;
use crate::types::*;
use crate::id_enums::{BuildSystemId, LanguageId, RuntimeId};
use std::path::Path;

pub struct DenoJsonParser;

impl ManifestParser for DenoJsonParser {
    fn filenames(&self) -> &[&str] {
        &["deno.json", "deno.jsonc"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Strip JSONC comments for parsing
        let clean = content
            .lines()
            .map(|l| {
                if let Some(idx) = l.find("//") {
                    &l[..idx]
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let _json: serde_json::Value = serde_json::from_str(&clean).ok()?;

        Some(Manifest {
            path: path.to_path_buf(),
            language: LanguageId::Deno,
            build_system: BuildSystemId::Deno,
            runtime: RuntimeId::Deno,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec!["deno".into(), "ca-certificates".into()],
                commands: vec!["deno install".into(), "deno task build".into()],
                member_transform: None,
                env: btree(&[("DENO_DIR", "/deno-dir")]),
                cache_dirs: vec!["/deno-dir".into()],
                artifacts: vec![
                    (".".into(), "/app".into()),
                    ("/deno-dir".into(), "/deno-dir".into()),
                ],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["deno".into(), "ca-certificates".into()],
                env: btree(&[("DENO_DIR", "/deno-dir")]),
                entrypoint: Some(
                    "deno run --allow-net --allow-read --allow-env main.ts".into(),
                ),
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(DenoJsonParser))
}
