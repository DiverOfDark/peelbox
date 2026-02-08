use crate::helpers::btree;
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use crate::id_enums::{FrameworkId, LanguageId};
use std::collections::BTreeMap;

// ── Laravel ─────────────────────────────────────────────────────────────────

super::simple_detector!(
    LaravelDetector,
    FrameworkId::Laravel,
    &[LanguageId::PHP],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name == "laravel/framework"),
    vec![8000],
    vec![],
    btree(&[("APP_ENV", "production")]),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(LaravelDetector))
}

// ── Symfony ─────────────────────────────────────────────────────────────────

pub struct SymfonyDetector;

impl FrameworkDetector for SymfonyDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Symfony
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::PHP]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name.starts_with("symfony/"))
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Symfony,
            default_ports: vec![8000],
            health_endpoints: vec!["/_health".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec![
                "/usr/bin/php".into(),
                "-S".into(),
                "0.0.0.0:8000".into(),
                "-t".into(),
                "/app/public".into(),
            ]),
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![
                ("vendor/".into(), "/app/vendor".into()),
                ("bin/".into(), "/app/bin".into()),
                ("public/".into(), "/app/public".into()),
                ("src/".into(), "/app/src".into()),
                ("config/".into(), "/app/config".into()),
            ],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SymfonyDetector))
}
