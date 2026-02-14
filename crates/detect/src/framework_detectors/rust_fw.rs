use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::types::Dependency;
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
