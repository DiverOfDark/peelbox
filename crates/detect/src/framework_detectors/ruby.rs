use crate::helpers::btree;
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use crate::id_enums::{FrameworkId, LanguageId};
use std::collections::BTreeMap;

// ── Rails ───────────────────────────────────────────────────────────────────

super::simple_detector!(
    RailsDetector,
    FrameworkId::Rails,
    &[LanguageId::Ruby],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "rails"),
    vec![3000],
    vec!["/up".into()],
    btree(&[("RAILS_ENV", "production")]),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(RailsDetector))
}

// ── Sinatra ─────────────────────────────────────────────────────────────────

pub struct SinatraDetector;

impl FrameworkDetector for SinatraDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Sinatra
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::Ruby]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "sinatra")
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Sinatra,
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
