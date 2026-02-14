use crate::helpers::btree;
use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const CSHARP: LanguageId = LanguageId::new("csharp");
const FSHARP: LanguageId = LanguageId::new("fsharp");

const ASP_NET_CORE: FrameworkId = FrameworkId::new("aspnet-core");
inventory::submit! { FrameworkMeta { slug: "aspnet-core", display_name: "ASP.NET Core", aliases: &[] } }

pub struct AspNetCoreDetector;

impl FrameworkDetector for AspNetCoreDetector {
    fn id(&self) -> FrameworkId {
        ASP_NET_CORE
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[CSHARP, FSHARP]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name.contains("Microsoft.AspNetCore"))
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: ASP_NET_CORE,
            default_ports: vec![5000],
            health_endpoints: vec!["/health".into(), "/healthz".into()],
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
