//! Framework detectors.
//!
//! Each detector checks dependencies for framework indicators and returns a `FrameworkContribution`.

use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use peelbox_stack::{FrameworkId, LanguageId};
use std::collections::BTreeMap;

fn btree(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

macro_rules! simple_detector {
    ($name:ident, $id:expr, $langs:expr, $detect_fn:expr, $ports:expr, $health:expr, $env:expr, $pkgs:expr) => {
        pub struct $name;
        impl FrameworkDetector for $name {
            fn id(&self) -> FrameworkId {
                $id
            }
            fn compatible_languages(&self) -> &[LanguageId] {
                $langs
            }
            fn detect(&self, deps: &[Dependency]) -> bool {
                ($detect_fn)(deps)
            }
            fn contribution(&self) -> FrameworkContribution {
                FrameworkContribution {
                    framework: $id,
                    default_ports: $ports,
                    health_endpoints: $health,
                    env_vars: $env,
                    runtime_packages: $pkgs,
                    runtime_command: None,
                    runtime_env: BTreeMap::new(),
                    workdir: None,
                    extra_copy: vec![],
                }
            }
        }
    };
}

// ── JVM Frameworks ──────────────────────────────────────────────────────────

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

simple_detector!(
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

simple_detector!(
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

simple_detector!(
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

// ── Node.js Frameworks ──────────────────────────────────────────────────────

simple_detector!(
    ExpressDetector,
    FrameworkId::Express,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "express"),
    vec![3000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

simple_detector!(
    NextJsDetector,
    FrameworkId::NextJs,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "next"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

simple_detector!(
    NestJsDetector,
    FrameworkId::NestJs,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "@nestjs/core"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

simple_detector!(
    FastifyDetector,
    FrameworkId::Fastify,
    &[LanguageId::JavaScript, LanguageId::TypeScript],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "fastify"),
    vec![3000],
    vec![],
    BTreeMap::new(),
    vec![]
);

// ── Python Frameworks ───────────────────────────────────────────────────────

simple_detector!(
    DjangoDetector,
    FrameworkId::Django,
    &[LanguageId::Python],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name == "django" || d.name == "Django"),
    vec![8000],
    vec!["/health/".into()],
    btree(&[("DJANGO_SETTINGS_MODULE", "config.settings.production")]),
    vec![]
);

pub struct FlaskDetector;

impl FrameworkDetector for FlaskDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Flask
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::Python]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| {
            d.name == "flask"
                || d.name == "Flask"
                || d.name.starts_with("flask==")
                || d.name.starts_with("Flask==")
        })
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Flask,
            default_ports: vec![5000],
            health_endpoints: vec!["/health".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec!["flask".into(), "run".into()]),
            runtime_env: btree(&[
                ("FLASK_APP", "/build/app.py"),
                ("FLASK_RUN_HOST", "0.0.0.0"),
                ("FLASK_RUN_PORT", "5000"),
                ("PATH", "/root/.local/bin:/usr/local/bin:/usr/bin:/bin"),
                ("PYTHONPATH", "/root/.local/lib/python3.14/site-packages"),
            ]),
            workdir: Some("/build".into()),
            extra_copy: vec![("/root/.local/".into(), "/root/.local".into())],
        }
    }
}

/// Flask detector for Poetry projects (uses .venv instead of --user install)
pub struct FlaskPoetryDetector;

impl FrameworkDetector for FlaskPoetryDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Flask
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::Python]
    }
    fn detect(&self, _deps: &[Dependency]) -> bool {
        // This is used specifically for Poetry projects; detection is handled externally
        false
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Flask,
            default_ports: vec![5000],
            health_endpoints: vec!["/health".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec!["flask".into(), "run".into()]),
            runtime_env: btree(&[
                ("FLASK_APP", "/build/app.py"),
                ("FLASK_RUN_HOST", "0.0.0.0"),
                ("FLASK_RUN_PORT", "5000"),
                ("VIRTUAL_ENV", "/build/.venv"),
                ("PATH", "/build/.venv/bin:/usr/local/bin:/usr/bin:/bin"),
            ]),
            workdir: Some("/build".into()),
            extra_copy: vec![],
        }
    }
}

simple_detector!(
    FastApiDetector,
    FrameworkId::FastApi,
    &[LanguageId::Python],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name == "fastapi" || d.name == "FastAPI"),
    vec![8000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

// ── Ruby Frameworks ─────────────────────────────────────────────────────────

simple_detector!(
    RailsDetector,
    FrameworkId::Rails,
    &[LanguageId::Ruby],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "rails"),
    vec![3000],
    vec!["/up".into()],
    btree(&[("RAILS_ENV", "production")]),
    vec![]
);

pub struct SinatraDetector;

impl FrameworkDetector for SinatraDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Sinatra
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::Ruby]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "sinatra")
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Sinatra,
            default_ports: vec![4567],
            health_endpoints: vec!["/health".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec![
                "bundle".into(),
                "exec".into(),
                "ruby".into(),
                "/app/app.rb".into(),
            ]),
            runtime_env: btree(&[
                ("BUNDLE_GEMFILE", "/app/Gemfile"),
                ("BUNDLE_PATH", "/app/vendor/bundle"),
            ]),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

// ── Rust Frameworks ─────────────────────────────────────────────────────────

simple_detector!(
    ActixWebDetector,
    FrameworkId::ActixWeb,
    &[LanguageId::Rust],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "actix-web"),
    vec![8080],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

simple_detector!(
    AxumDetector,
    FrameworkId::Axum,
    &[LanguageId::Rust],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "axum"),
    vec![3000],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

// ── Go Frameworks ───────────────────────────────────────────────────────────

simple_detector!(
    GinDetector,
    FrameworkId::Gin,
    &[LanguageId::Go],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("github.com/gin-gonic/gin")),
    vec![8080],
    vec!["/health".into()],
    BTreeMap::new(),
    vec![]
);

simple_detector!(
    EchoDetector,
    FrameworkId::Echo,
    &[LanguageId::Go],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name.contains("github.com/labstack/echo")),
    vec![8080],
    vec![],
    BTreeMap::new(),
    vec![]
);

// ── .NET Frameworks ─────────────────────────────────────────────────────────

pub struct AspNetCoreDetector;

impl FrameworkDetector for AspNetCoreDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::AspNetCore
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::CSharp, LanguageId::FSharp]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter()
            .any(|d| d.name.contains("Microsoft.AspNetCore"))
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::AspNetCore,
            default_ports: vec![5000],
            health_endpoints: vec!["/health".into()],
            env_vars: btree(&[("ASPNETCORE_URLS", "http://0.0.0.0:5000")]),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

// ── PHP Frameworks ──────────────────────────────────────────────────────────

simple_detector!(
    LaravelDetector,
    FrameworkId::Laravel,
    &[LanguageId::PHP],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name == "laravel/framework"),
    vec![8000],
    vec![],
    btree(&[("APP_ENV", "production")]),
    vec![]
);

pub struct SymfonyDetector;

impl FrameworkDetector for SymfonyDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Symfony
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::PHP]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name.starts_with("symfony/"))
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Symfony,
            default_ports: vec![8000],
            health_endpoints: vec!["/_health".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec![
                "/usr/bin/php".into(),
                "-S".into(),
                "0.0.0.0:8000".into(),
                "-t".into(),
                "/app/public".into(),
            ]),
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![
                ("vendor/".into(), "/app/vendor".into()),
                ("bin/".into(), "/app/bin".into()),
                ("public/".into(), "/app/public".into()),
                ("src/".into(), "/app/src".into()),
                ("config/".into(), "/app/config".into()),
            ],
        }
    }
}

// ── Elixir Frameworks ───────────────────────────────────────────────────────

simple_detector!(
    PhoenixDetector,
    FrameworkId::Phoenix,
    &[LanguageId::Elixir],
    |deps: &[Dependency]| deps.iter().any(|d| d.name == "phoenix"),
    vec![4000],
    vec![],
    BTreeMap::new(),
    vec![]
);

// ── Zig Frameworks ──────────────────────────────────────────────────────────

pub struct ZapDetector;

impl FrameworkDetector for ZapDetector {
    fn id(&self) -> FrameworkId {
        FrameworkId::Custom("Zap".into())
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[LanguageId::Zig]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "zap")
    }
    fn contribution(&self) -> FrameworkContribution {
        FrameworkContribution {
            framework: FrameworkId::Custom("Zap".into()),
            default_ports: vec![3000],
            health_endpoints: vec!["/health".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: None,
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DepScope;

    fn make_dep(name: &str) -> Dependency {
        Dependency {
            name: name.to_string(),
            version: None,
            scope: DepScope::Runtime,
            is_internal: false,
        }
    }

    #[test]
    fn test_spring_boot_detection() {
        let detector = SpringBootDetector;
        let deps = vec![make_dep(
            "org.springframework.boot:spring-boot-starter-web",
        )];
        assert!(detector.detect(&deps));
        let contrib = detector.contribution();
        assert_eq!(contrib.framework, FrameworkId::SpringBoot);
        assert_eq!(contrib.default_ports, vec![8080]);
    }

    #[test]
    fn test_express_detection() {
        let detector = ExpressDetector;
        let deps = vec![make_dep("express"), make_dep("body-parser")];
        assert!(detector.detect(&deps));
    }

    #[test]
    fn test_no_false_positive() {
        let detector = SpringBootDetector;
        let deps = vec![make_dep("express"), make_dep("react")];
        assert!(!detector.detect(&deps));
    }

    #[test]
    fn test_django_detection() {
        let detector = DjangoDetector;
        let deps = vec![make_dep("django")];
        assert!(detector.detect(&deps));
    }

    #[test]
    fn test_gin_detection() {
        let detector = GinDetector;
        let deps = vec![make_dep("github.com/gin-gonic/gin")];
        assert!(detector.detect(&deps));
    }
}
