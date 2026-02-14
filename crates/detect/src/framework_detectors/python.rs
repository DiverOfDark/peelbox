use crate::helpers::btree;
use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const PYTHON: LanguageId = LanguageId::new("python");

// ── Django ──────────────────────────────────────────────────────────────────

const DJANGO: FrameworkId = FrameworkId::new("django");
inventory::submit! { FrameworkMeta { slug: "django", display_name: "Django", aliases: &[] } }

super::simple_detector!(
    DjangoDetector,
    DJANGO,
    &[PYTHON],
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

const FLASK: FrameworkId = FrameworkId::new("flask");
inventory::submit! { FrameworkMeta { slug: "flask", display_name: "Flask", aliases: &[] } }

pub struct FlaskDetector;

impl FrameworkDetector for FlaskDetector {
    fn id(&self) -> FrameworkId {
        FLASK
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PYTHON]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter().any(|d| {
            d.name == "flask"
                || d.name == "Flask"
                || d.name.starts_with("flask==")
                || d.name.starts_with("Flask==")
        })
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: FLASK,
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
        FLASK
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PYTHON]
    }
    fn detect(&self, _deps: &[Dependency]) -> bool {
        // This is used specifically for Poetry projects; detection is handled externally
        false
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: FLASK,
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

const FAST_API: FrameworkId = FrameworkId::new("fastapi");
inventory::submit! { FrameworkMeta { slug: "fastapi", display_name: "FastAPI", aliases: &[] } }

super::simple_detector!(
    FastApiDetector,
    FAST_API,
    &[PYTHON],
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
