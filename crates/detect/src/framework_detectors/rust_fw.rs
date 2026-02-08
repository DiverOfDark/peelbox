use crate::types::Dependency;
use peelbox_stack::{FrameworkId, LanguageId};
use std::collections::BTreeMap;

super::simple_detector!(
    ActixWebDetector,
    FrameworkId::ActixWeb,
    &[LanguageId::Rust],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "actix-web"),
    vec![8080],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ActixWebDetector))
}

super::simple_detector!(
    AxumDetector,
    FrameworkId::Axum,
    &[LanguageId::Rust],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "axum"),
    vec![3000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AxumDetector))
}
