use crate::helpers::btree;
use crate::ids::{
    BuildSystemId, BuildSystemMeta, LanguageId, LanguageMeta, RuntimeId, RuntimeMeta,
};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const RUBY: LanguageId = LanguageId::new("ruby");
const BUNDLER: BuildSystemId = BuildSystemId::new("bundler");
const RUBY_RT: RuntimeId = RuntimeId::new("ruby");

inventory::submit! {
    LanguageMeta { slug: "ruby", display_name: "Ruby", aliases: &[] }
}
inventory::submit! {
    BuildSystemMeta { slug: "bundler", display_name: "Bundler", aliases: &["bundler"] }
}
inventory::submit! {
    RuntimeMeta { slug: "ruby", display_name: "Ruby", aliases: &["ruby"] }
}

pub struct GemfileParser;

impl ManifestParser for GemfileParser {
    fn filenames(&self) -> &[&str] {
        &["Gemfile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("gem ") && !content.contains("source ") {
            return None;
        }

        let deps: Vec<Dependency> = content
            .lines()
            .filter_map(|l| {
                let trimmed = l.trim();
                if trimmed.starts_with("gem ") {
                    let name = trimmed
                        .trim_start_matches("gem ")
                        .split(',')
                        .next()?
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string();
                    Some(Dependency {
                        name,
                        version: None,
                        scope: DepScope::Runtime,
                        is_internal: false,
                    })
                } else {
                    None
                }
            })
            .collect();

        Some(Manifest {
            path: path.to_path_buf(),
            language: RUBY,
            build_system: BUNDLER,
            runtime: RUBY_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: deps,
            build: BuildSpec {
                packages: vec![
                    "ruby".into(),
                    "ruby-dev".into(),
                    "ruby-bundler".into(),
                    "build-base".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["bundle install".into()],
                member_transform: None,
                env: btree(&[
                    ("BUNDLE_DEPLOYMENT", "false"),
                    ("BUNDLE_PATH", "vendor/bundle"),
                ]),
                cache_dirs: vec![".bundle".into(), "vendor".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "ruby".into(),
                    "ruby-bundler".into(),
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: BTreeMap::new(),
                entrypoint: None, // Set by framework detector (Sinatra/Rails)
                workdir: Some("/app".into()),
                ports: vec![3000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(GemfileParser))
}
