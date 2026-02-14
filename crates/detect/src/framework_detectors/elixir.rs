use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::types::Dependency;
use std::collections::BTreeMap;

const ELIXIR: LanguageId = LanguageId::new("elixir");

const PHOENIX: FrameworkId = FrameworkId::new("phoenix");
inventory::submit! { FrameworkMeta { slug: "phoenix", display_name: "Phoenix", aliases: &[] } }

super::simple_detector!(
    PhoenixDetector,
    PHOENIX,
    &[ELIXIR],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "phoenix"),
    vec![4000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(PhoenixDetector))
}
