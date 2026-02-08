use crate::helpers::btree;
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use crate::id_enums::{FrameworkId, LanguageId};
use std::collections::BTreeMap;

// ── Spring Boot ─────────────────────────────────────────────────────────────

pub struct SpringBootDetector;

impl FrameworkDetector for SpringBootDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::SpringBoot
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::Java, LanguageId::Kotlin]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| {
            d.name
                .contains("org.springframework.boot:spring-boot-starter")
        })
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::SpringBoot,
            default_ports: vec![8080],
            health_endpoints: vec!["/actuator/health".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            // Spring Boot runtime needs JAVA_HOME and PATH
            runtime_env: btree(&[
                ("JAVA_HOME", "/usr/lib/jvm/java-17-openjdk"),
                ("PATH", "/usr/lib/jvm/java-17-openjdk/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"),
            ]),
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

super::simple_detector!(
    QuarkusDetector,
    FrameworkId::Quarkus,
    &[LanguageId::Java, LanguageId::Kotlin],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("io.quarkus:quarkus-")),
    vec![8080],
    vec!["/q/health".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(QuarkusDetector))
}

// ── Micronaut ───────────────────────────────────────────────────────────────

super::simple_detector!(
    MicronautDetector,
    FrameworkId::Micronaut,
    &[LanguageId::Java, LanguageId::Kotlin],
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

super::simple_detector!(
    KtorDetector,
    FrameworkId::Ktor,
    &[LanguageId::Kotlin],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("io.ktor:ktor-")),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(KtorDetector))
}
