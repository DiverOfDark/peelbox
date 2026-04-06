use crate::helpers::btree;
use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const ELIXIR: LanguageId = LanguageId::new("elixir");

const PHOENIX: FrameworkId = FrameworkId::new("phoenix");
inventory::submit! { FrameworkMeta { slug: "phoenix", display_name: "Phoenix", aliases: &[] } }

pub struct PhoenixDetector;

impl FrameworkDetector for PhoenixDetector {
    fn id(&self) -> FrameworkId {
        PHOENIX
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[ELIXIR]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "phoenix")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: PHOENIX,
            default_ports: vec![4000],
            health_endpoints: vec!["/".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: None,
            // Phoenix requires SECRET_KEY_BASE (≥64 bytes) for cookie signing.
            // Provide a default so containers start without manual configuration.
            runtime_env: btree(&[(
                "SECRET_KEY_BASE",
                "please-change-me-in-production-this-is-a-placeholder-key-that-is-at-least-64-bytes-long",
            )]),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(PhoenixDetector))
}
