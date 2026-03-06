use crate::helpers::btree;
use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const RUBY: LanguageId = LanguageId::new("ruby");

// ── Rails ───────────────────────────────────────────────────────────────────

const RAILS: FrameworkId = FrameworkId::new("rails");
inventory::submit! { FrameworkMeta { slug: "rails", display_name: "Rails", aliases: &[] } }

pub struct RailsDetector;

impl FrameworkDetector for RailsDetector {
    fn id(&self) -> FrameworkId {
        RAILS
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[RUBY]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "rails")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: RAILS,
            default_ports: vec![3000],
            health_endpoints: vec!["/".into(), "/up".into()],
            env_vars: btree(&[("RAILS_ENV", "production")]),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: btree(&[
                ("BUNDLE_GEMFILE", "/app/Gemfile"),
                ("BUNDLE_PATH", "/app/vendor/bundle"),
                ("RAILS_ENV", "production"),
                ("RAILS_LOG_TO_STDOUT", "true"),
                ("RAILS_SERVE_STATIC_FILES", "true"),
                ("SECRET_KEY_BASE", "please-change-me-in-production-this-is-a-placeholder-key-that-is-at-least-64-bytes-long"),
            ]),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(RailsDetector))
}

// ── Sinatra ─────────────────────────────────────────────────────────────────

const SINATRA: FrameworkId = FrameworkId::new("sinatra");
inventory::submit! { FrameworkMeta { slug: "sinatra", display_name: "Sinatra", aliases: &[] } }

pub struct SinatraDetector;

impl FrameworkDetector for SinatraDetector {
    fn id(&self) -> FrameworkId {
        SINATRA
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[RUBY]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "sinatra")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: SINATRA,
            default_ports: vec![4567],
            health_endpoints: vec!["/".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec![
                "bundle".into(),
                "exec".into(),
                "ruby".into(),
                "/app/app.rb".into(),
            ]),
            runtime_env: btree(&[
                ("BUNDLE_GEMFILE", "/app/Gemfile"),
                ("BUNDLE_PATH", "/app/vendor/bundle"),
                ("RACK_ENV", "production"),
            ]),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SinatraDetector))
}
