use crate::id_enums::{FrameworkId, LanguageId};
use crate::types::Dependency;
use std::collections::BTreeMap;

super::simple_detector!(
    GinDetector,
    FrameworkId::Gin,
    &[LanguageId::Go],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("github.com/gin-gonic/gin")),
    vec![8080],
    vec!["/health".into(), "/healthz".into(), "/ping".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(GinDetector))
}

super::simple_detector!(
    EchoDetector,
    FrameworkId::Echo,
    &[LanguageId::Go],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("github.com/labstack/echo")),
    vec![8080],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(EchoDetector))
}
