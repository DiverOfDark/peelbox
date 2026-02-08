use crate::types::Dependency;
use crate::id_enums::{FrameworkId, LanguageId};
use std::collections::BTreeMap;

super::simple_detector!(
    ExpressDetector,
    FrameworkId::Express,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "express"),
    vec![3000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(ExpressDetector))
}

super::simple_detector!(
    NextJsDetector,
    FrameworkId::NextJs,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "next"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NextJsDetector))
}

super::simple_detector!(
    NestJsDetector,
    FrameworkId::NestJs,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@nestjs/core"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(NestJsDetector))
}

super::simple_detector!(
    FastifyDetector,
    FrameworkId::Fastify,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "fastify"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FastifyDetector))
}
