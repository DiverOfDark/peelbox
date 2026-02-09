use crate::helpers::btree;
use crate::id_enums::{FrameworkId, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

// ── Django ──────────────────────────────────────────────────────────────────

super::simple_detector!(
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

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(DjangoDetector))
}

// ── Flask ───────────────────────────────────────────────────────────────────

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
            extra_copy: vec![
                (".".into(), "/build".into()),
                ("/root/.local/".into(), "/root/.local".into()),
            ],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FlaskDetector))
}

// ── Flask (Poetry) ──────────────────────────────────────────────────────────

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

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FlaskPoetryDetector))
}

// ── FastAPI ─────────────────────────────────────────────────────────────────

super::simple_detector!(
    FastApiDetector,
    FrameworkId::FastApi,
    &[LanguageId::Python],
    |deps: &[Dependency]| deps
        .iter()
        .any(|d| d.name == "fastapi" || d.name == "FastAPI"),
    vec![8000],
    vec!["/health".into(), "/healthz".into()],
    BTreeMap::new(),
    vec![]
);

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FastApiDetector))
}
