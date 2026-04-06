use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const RUST: LanguageId = LanguageId::new("rust");

const ACTIX_WEB: FrameworkId = FrameworkId::new("actix-web");
inventory::submit! { FrameworkMeta { slug: "actix-web", display_name: "Actix Web", aliases: &[] } }

super::simple_detector!(
    ActixWebDetector,
    ACTIX_WEB,
    &[RUST],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "actix-web"),
    vec![8080],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ActixWebDetector))
}

const AXUM: FrameworkId = FrameworkId::new("axum");
inventory::submit! { FrameworkMeta { slug: "axum", display_name: "Axum", aliases: &[] } }

super::simple_detector!(
    AxumDetector,
    AXUM,
    &[RUST],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "axum"),
    vec![3000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AxumDetector))
}

const ROCKET: FrameworkId = FrameworkId::new("rocket");
inventory::submit! { FrameworkMeta { slug: "rocket", display_name: "Rocket", aliases: &[] } }

pub struct RocketDetector;
impl FrameworkDetector for RocketDetector {
    fn id(&self) -> FrameworkId {
        ROCKET
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[RUST]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "rocket")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: ROCKET,
            default_ports: vec![8000],
            health_endpoints: vec!["/".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: BTreeMap::from([("ROCKET_ADDRESS".into(), "0.0.0.0".into())]),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(RocketDetector))
}
