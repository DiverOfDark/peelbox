use crate::helpers::btree;
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use peelbox_stack::{FrameworkId, LanguageId};
use std::collections::BTreeMap;

pub struct AspNetCoreDetector;

impl FrameworkDetector for AspNetCoreDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::AspNetCore
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::CSharp, LanguageId::FSharp]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter()
            .any(|d| d.name.contains("Microsoft.AspNetCore"))
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::AspNetCore,
            default_ports: vec![5000],
            health_endpoints: vec!["/health".into()],
            env_vars: btree(&[("ASPNETCORE_URLS", "http://0.0.0.0:5000")]),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AspNetCoreDetector))
}
