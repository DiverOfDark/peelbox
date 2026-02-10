use crate::id_enums::{FrameworkId, LanguageId};
use crate::types::Dependency;
use std::collections::BTreeMap;

super::simple_detector!(
    PhoenixDetector,
    FrameworkId::Phoenix,
    &[LanguageId::Elixir],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "phoenix"),
    vec![4000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(PhoenixDetector))
}
