use crate::helpers::btree;
use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const PHP: LanguageId = LanguageId::new("php");

// ── Laravel ─────────────────────────────────────────────────────────────────

const LARAVEL: FrameworkId = FrameworkId::new("laravel");
inventory::submit! { FrameworkMeta { slug: "laravel", display_name: "Laravel", aliases: &[] } }

pub struct LaravelDetector;

impl FrameworkDetector for LaravelDetector {
    fn id(&self) -> FrameworkId {
        LARAVEL
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PHP]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name == "laravel/framework")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: LARAVEL,
            default_ports: vec![8000],
            health_endpoints: vec!["/".into()],
            env_vars: btree(&[("APP_ENV", "production")]),
            runtime_packages: vec![],
            runtime_command: Some(vec![
                "php".into(),
                "artisan".into(),
                "serve".into(),
                "--host=0.0.0.0".into(),
                "--port=8000".into(),
            ]),
            runtime_env: btree(&[
                ("SESSION_DRIVER", "file"),
                ("CACHE_STORE", "file"),
            ]),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(LaravelDetector))
}

// ── Symfony ─────────────────────────────────────────────────────────────────

const SYMFONY: FrameworkId = FrameworkId::new("symfony");
inventory::submit! { FrameworkMeta { slug: "symfony", display_name: "Symfony", aliases: &[] } }

pub struct SymfonyDetector;

impl FrameworkDetector for SymfonyDetector {
    fn id(&self) -> FrameworkId {
        SYMFONY
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PHP]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| d.name.starts_with("symfony/"))
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: SYMFONY,
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

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(SymfonyDetector))
}
