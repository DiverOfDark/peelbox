use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use peelbox_stack::{FrameworkId, LanguageId};
use std::collections::BTreeMap;

pub struct ZapDetector;

impl FrameworkDetector for ZapDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Custom("Zap".into())
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::Zig]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "zap")
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Custom("Zap".into()),
            default_ports: vec![3000],
            health_endpoints: vec!["/health".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ZapDetector))
}
