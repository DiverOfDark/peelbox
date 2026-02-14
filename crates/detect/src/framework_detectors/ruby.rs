use crate::helpers::btree;
use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const RUBY: LanguageId = LanguageId::new("ruby");

// ── Rails ───────────────────────────────────────────────────────────────────

const RAILS: FrameworkId = FrameworkId::new("rails");
inventory::submit! { FrameworkMeta { slug: "rails", display_name: "Rails", aliases: &[] } }

super::simple_detector!(
    RailsDetector,
    RAILS,
    &[RUBY],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "rails"),
    vec![3000],
    vec!["/health".into(), "/up".into()],
    btree(&[("RAILS_ENV", "production")]),
    vec![]
);

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
            health_endpoints: vec!["/health".into()],
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
            ]),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SinatraDetector))
}
