use crate::ids::{BuildSystemId, BuildSystemMeta, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const CPP: LanguageId = LanguageId::new("c++");
const MESON: BuildSystemId = BuildSystemId::new("meson");
const NATIVE: RuntimeId = RuntimeId::new("native");

inventory::submit! {
    BuildSystemMeta { slug: "meson", display_name: "Meson", aliases: &["meson"] }
}

pub struct MesonBuildParser;

impl ManifestParser for MesonBuildParser {
    fn filenames(&self) -> &[&str] {
        &["meson.build"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("project(") {
            return None;
        }

        let name_re = regex::Regex::new(r"project\s*\(\s*'([^']+)'").ok()?;
        let name = name_re
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "app".to_string());

        Some(Manifest {
            path: path.to_path_buf(),
            language: CPP,
            build_system: MESON,
            runtime: NATIVE,
            package: Some(Package {
                name: name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: vec![
                    "meson".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec![
                    "meson setup build --buildtype=release --wipe 2>/dev/null || meson setup build --buildtype=release".into(),
                    "meson compile -C build".into(),
                ],
                member_transform: None,
                env: BTreeMap::new(),
                cache_dirs: vec!["build/".into()],
                artifacts: vec![(format!("build/{}", name), format!("/app/{}", name))],
                            build_image: None,
},
            runtime_config: RuntimeSpec {
                packages: vec!["ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", name)),
                workdir: Some("/app".into()),
                ports: vec![8080],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(MesonBuildParser))
}
