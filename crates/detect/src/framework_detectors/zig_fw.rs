use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const ZIG: LanguageId = LanguageId::new("zig");

const ZAP: FrameworkId = FrameworkId::new("zap");
inventory::submit! { FrameworkMeta { slug: "zap", display_name: "Zap", aliases: &[] } }

pub struct ZapDetector;

impl FrameworkDetector for ZapDetector {
    fn id(&self) -> FrameworkId {
        ZAP
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[ZIG]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "zap")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: ZAP,
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
