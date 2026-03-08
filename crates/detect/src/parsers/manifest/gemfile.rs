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

        let has_sqlite = deps.iter().any(|d| d.name == "sqlite3");
        let has_pg = deps.iter().any(|d| d.name == "pg");
        let has_mysql = deps.iter().any(|d| d.name == "mysql2");
        let has_charlock = deps.iter().any(|d| d.name == "charlock_holmes");
        let has_nokogiri = deps.iter().any(|d| d.name == "nokogiri");

        let mut build_packages = vec![
            "ruby".into(),
            "ruby-dev".into(),
            "ruby-bundler".into(),
            "build-base".into(),
            "glibc-dev".into(),
            "linux-headers".into(),
            "libffi-dev".into(),
            "yaml-dev".into(),
            "pkgconf".into(),
            "ca-certificates".into(),
        ];
        if has_sqlite {
            build_packages.push("sqlite-dev".into());
        }
        if has_pg {
            build_packages.push("postgresql-dev".into());
        }
        if has_mysql {
            build_packages.push("mariadb-connector-c-dev".into());
        }
        if has_charlock {
            build_packages.push("icu-dev".into());
        }
        if has_nokogiri {
            build_packages.push("libxml2-dev".into());
            build_packages.push("libxslt-dev".into());
        }

        let mut runtime_packages: Vec<String> = vec![
            "ruby".into(),
            "ruby-bundler".into(),
            "libgcc".into(),
            "libstdc++".into(),
            "busybox".into(),
            "tzdata".into(),
            "ca-certificates".into(),
        ];
        if has_sqlite {
            runtime_packages.push("sqlite-libs".into());
        }
        if has_pg {
            runtime_packages.push("libpq".into());
        }
        if has_mysql {
            runtime_packages.push("mariadb-connector-c".into());
        }
        if has_charlock {
            runtime_packages.push("icu".into());
        }
        if has_nokogiri {
            runtime_packages.push("libxml2".into());
            runtime_packages.push("libxslt".into());
        }

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
                packages: build_packages,
                commands: vec![
                    // Remove exact ruby version constraint from Gemfile.
                    // Add base64 gem (removed from Ruby 3.4 stdlib, needed by older rack/etc).
                    // If bundle install fails (e.g., native gem incompatible with current
                    // Ruby), extract failing gem name and update just that gem.
                    "sed -i '/^ruby /d' Gemfile && (grep -q \"gem.*'base64'\" Gemfile || echo \"gem 'base64'\" >> Gemfile) && bundle install || { FAILED_GEM=$(bundle install 2>&1 | sed -n 's/.*error occurred while installing \\([^ ]*\\).*/\\1/p'); [ -n \"$FAILED_GEM\" ] && bundle update $FAILED_GEM && bundle install || bundle update && bundle install; }".into(),
                ],
                member_transform: None,
                env: btree(&[
                    ("BUNDLE_DEPLOYMENT", "false"),
                    ("BUNDLE_PATH", "vendor/bundle"),
                ]),
                cache_dirs: vec![".bundle".into(), "vendor".into()],
                artifacts: vec![(".".into(), "/app".into())],
            },
            runtime_config: RuntimeSpec {
                packages: runtime_packages,
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
