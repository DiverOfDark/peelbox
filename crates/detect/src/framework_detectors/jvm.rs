use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const JAVA: LanguageId = LanguageId::new("java");
const KOTLIN: LanguageId = LanguageId::new("kotlin");
const SCALA: LanguageId = LanguageId::new("scala");
const CLOJURE: LanguageId = LanguageId::new("clojure");

// ── Spring Boot ─────────────────────────────────────────────────────────────

const SPRING_BOOT: FrameworkId = FrameworkId::new("spring-boot");
inventory::submit! { FrameworkMeta { slug: "spring-boot", display_name: "Spring Boot", aliases: &[] } }

pub struct SpringBootDetector;

impl FrameworkDetector for SpringBootDetector {
    fn id(&self) -> FrameworkId {
        SPRING_BOOT
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[JAVA, KOTLIN, SCALA]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| {
            d.name
                .contains("org.springframework.boot:spring-boot-starter")
        })
    }
    fn contribution(&self, deps: &[Dependency]) -> FrameworkContribution {
        let has_actuator = deps.iter().any(|d| {
            d.name.contains("spring-boot-starter-actuator")
                || d.name.contains("spring-boot-actuator")
        });
        let health_endpoints = if has_actuator {
            vec![
                "/actuator/health".into(),
                "/actuator/health/liveness".into(),
                "/actuator/health/readiness".into(),
            ]
        } else {
            vec!["/health".into()]
        };
        FrameworkContribution {
            framework: SPRING_BOOT,
            default_ports: vec![8080],
            health_endpoints,
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_env: BTreeMap::new(),
            runtime_command: None,
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SpringBootDetector))
}

// ── Quarkus ─────────────────────────────────────────────────────────────────

const QUARKUS: FrameworkId = FrameworkId::new("quarkus");
inventory::submit! { FrameworkMeta { slug: "quarkus", display_name: "Quarkus", aliases: &[] } }

super::simple_detector!(
    QuarkusDetector,
    QUARKUS,
    &[JAVA, KOTLIN, SCALA],
    |deps: &[Dependency]| deps.iter().any(|d| d.name.contains("io.quarkus:quarkus-")),
    vec![8080],
    vec!["/q/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(QuarkusDetector))
}

// ── Micronaut ───────────────────────────────────────────────────────────────

const MICRONAUT: FrameworkId = FrameworkId::new("micronaut");
inventory::submit! { FrameworkMeta { slug: "micronaut", display_name: "Micronaut", aliases: &[] } }

super::simple_detector!(
    MicronautDetector,
    MICRONAUT,
    &[JAVA, KOTLIN, SCALA],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("io.micronaut:micronaut-")),
    vec![8080],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(MicronautDetector))
}

// ── Ktor ────────────────────────────────────────────────────────────────────

const KTOR: FrameworkId = FrameworkId::new("ktor");
inventory::submit! { FrameworkMeta { slug: "ktor", display_name: "Ktor", aliases: &[] } }

super::simple_detector!(
    KtorDetector,
    KTOR,
    &[KOTLIN],
    |deps: &[Dependency]| deps.iter().any(|d| d.name.contains("io.ktor:ktor-")),
    vec![8080],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(KtorDetector))
}

// ── Akka HTTP ────────────────────────────────────────────────────────────

const AKKA_HTTP: FrameworkId = FrameworkId::new("akka-http");
inventory::submit! { FrameworkMeta { slug: "akka-http", display_name: "Akka HTTP", aliases: &[] } }

super::simple_detector!(
    AkkaHttpDetector,
    AKKA_HTTP,
    &[JAVA, SCALA],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("akka-http") || d.name.contains("pekko-http")),
    vec![8080],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(AkkaHttpDetector))
}

// ── Ring (Clojure) ─────────────────────────────────────────────────────────

const RING: FrameworkId = FrameworkId::new("ring");
inventory::submit! { FrameworkMeta { slug: "ring", display_name: "Ring", aliases: &[] } }

super::simple_detector!(
    RingDetector,
    RING,
    &[CLOJURE],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name == "ring/ring-core" || d.name == "ring/ring-jetty-adapter"),
    vec![3000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(RingDetector))
}
// ── Play Framework ───────────────────────────────────────────────────────

const PLAY: FrameworkId = FrameworkId::new("play");
inventory::submit! { FrameworkMeta { slug: "play", display_name: "Play Framework", aliases: &[] } }

super::simple_detector!(
    PlayDetector,
    PLAY,
    &[JAVA, SCALA],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("com.typesafe.play:play") || d.name.contains("playframework")),
    vec![9000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(PlayDetector))
}
