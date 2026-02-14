use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::types::Dependency;
use std::collections::BTreeMap;

const GO: LanguageId = LanguageId::new("go");

const GIN: FrameworkId = FrameworkId::new("gin");
inventory::submit! { FrameworkMeta { slug: "gin", display_name: "Gin", aliases: &[] } }

super::simple_detector!(
    GinDetector,
    GIN,
    &[GO],
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

const ECHO: FrameworkId = FrameworkId::new("echo");
inventory::submit! { FrameworkMeta { slug: "echo", display_name: "Echo", aliases: &[] } }

super::simple_detector!(
    EchoDetector,
    ECHO,
    &[GO],
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
