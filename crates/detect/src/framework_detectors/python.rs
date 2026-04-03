use crate::helpers::btree;
use crate::ids::{FrameworkId, FrameworkMeta, LanguageId};
use crate::traits::FrameworkDetector;
use crate::types::{Dependency, FrameworkContribution};
use std::collections::BTreeMap;

const PYTHON: LanguageId = LanguageId::new("python");

// ── Django ──────────────────────────────────────────────────────────────────

const DJANGO: FrameworkId = FrameworkId::new("django");
inventory::submit! { FrameworkMeta { slug: "django", display_name: "Django", aliases: &[] } }

pub struct DjangoDetector;

impl FrameworkDetector for DjangoDetector {
    fn id(&self) -> FrameworkId {
        DJANGO
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PYTHON]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter()
            .any(|d| d.name == "django" || d.name == "Django")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: DJANGO,
            default_ports: vec![8000],
            health_endpoints: vec!["/health/".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec![
                "python".into(),
                "manage.py".into(),
                "runserver".into(),
                "0.0.0.0:8000".into(),
            ]),
            runtime_env: BTreeMap::new(),
            workdir: None,
            extra_copy: vec![],
        }
    }
}

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
            health_endpoints: vec!["/".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec!["flask".into(), "run".into()]),
            runtime_env: btree(&[
                ("FLASK_APP", "/app/app.py"),
                ("FLASK_RUN_HOST", "0.0.0.0"),
                ("FLASK_RUN_PORT", "5000"),
                ("PATH", "/root/.local/bin:/usr/local/bin:/usr/bin:/bin"),
                ("PYTHONUSERBASE", "/root/.local"),
            ]),
            workdir: Some("/app".into()),
            extra_copy: vec![
                (".".into(), "/app".into()),
                ("/root/.local/".into(), "/root/.local".into()),
            ],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FlaskDetector))
}

// ── Flask (venv-based: Poetry, PDM, UV) ────────────────────────────────────

/// Shared contribution for Flask detectors that use a virtual environment.
///
/// Poetry, PDM, and UV all install into a `.venv` directory rather than using
/// `--user` (pip) installs. The only difference between them is the workdir
/// (`/app` for Poetry/PDM/UV), which determines where `FLASK_APP`,
/// `VIRTUAL_ENV`, and `PATH` point. This helper eliminates the duplication.
fn flask_venv_contribution(workdir: &str) -> FrameworkContribution {
    FrameworkContribution {
        framework: FLASK,
        default_ports: vec![5000],
        health_endpoints: vec!["/".into()],
        env_vars: BTreeMap::new(),
        runtime_packages: vec![],
        runtime_command: Some(vec!["flask".into(), "run".into()]),
        runtime_env: btree(&[
            ("FLASK_APP", &format!("{workdir}/app.py")),
            ("FLASK_RUN_HOST", "0.0.0.0"),
            ("FLASK_RUN_PORT", "5000"),
            ("VIRTUAL_ENV", &format!("{workdir}/.venv")),
            (
                "PATH",
                &format!("{workdir}/.venv/bin:/usr/local/bin:/usr/bin:/bin"),
            ),
        ]),
        workdir: Some(workdir.into()),
        extra_copy: vec![],
    }
}

/// Flask detector for Poetry projects (uses .venv instead of --user install).
///
/// `detect()` returns `false` by design. This detector is not selected via
/// dependency scanning; instead, the Poetry build-system profile declares
/// `preferred_framework_env_keys: &["VIRTUAL_ENV"]`, and the pipeline's
/// framework-selection logic picks this contribution when the build system
/// is Poetry and the base Flask framework was already detected.
pub struct FlaskPoetryDetector;

impl FrameworkDetector for FlaskPoetryDetector {
    fn id(&self) -> FrameworkId {
        FLASK
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PYTHON]
    }
    fn detect(&self, _deps: &[Dependency]) -> bool {
        false
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        flask_venv_contribution("/app")
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FlaskPoetryDetector))
}

/// Flask detector for PDM projects (uses .venv instead of --user install).
///
/// `detect()` returns `false` by design. This detector is not selected via
/// dependency scanning; instead, the PDM build-system profile declares
/// `preferred_framework_env_keys: &["VIRTUAL_ENV"]`, and the pipeline's
/// framework-selection logic picks this contribution when the build system
/// is PDM and the base Flask framework was already detected.
pub struct FlaskPdmDetector;

impl FrameworkDetector for FlaskPdmDetector {
    fn id(&self) -> FrameworkId {
        FLASK
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PYTHON]
    }
    fn detect(&self, _deps: &[Dependency]) -> bool {
        false
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        flask_venv_contribution("/app")
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FlaskPdmDetector))
}

/// Flask detector for UV projects (uses .venv in /app workdir).
///
/// `detect()` returns `false` by design. This detector is not selected via
/// dependency scanning; instead, the UV build-system profile declares
/// `preferred_framework_env_keys: &["VIRTUAL_ENV"]`, and the pipeline's
/// framework-selection logic picks this contribution when the build system
/// is UV and the base Flask framework was already detected.
pub struct FlaskUvDetector;

impl FrameworkDetector for FlaskUvDetector {
    fn id(&self) -> FrameworkId {
        FLASK
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PYTHON]
    }
    fn detect(&self, _deps: &[Dependency]) -> bool {
        false
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        flask_venv_contribution("/app")
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FlaskUvDetector))
}

// ── FastAPI ─────────────────────────────────────────────────────────────────

const FAST_API: FrameworkId = FrameworkId::new("fastapi");
inventory::submit! { FrameworkMeta { slug: "fastapi", display_name: "FastAPI", aliases: &[] } }

pub struct FastApiDetector;

impl FrameworkDetector for FastApiDetector {
    fn id(&self) -> FrameworkId {
        FAST_API
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PYTHON]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter()
            .any(|d| d.name == "fastapi" || d.name == "FastAPI")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: FAST_API,
            default_ports: vec![8000],
            health_endpoints: vec!["/health".into(), "/healthz".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec![
                "uvicorn".into(),
                "main:app".into(),
                "--host".into(),
                "0.0.0.0".into(),
                "--port".into(),
                "8000".into(),
            ]),
            runtime_env: btree(&[
                ("PATH", "/app/.venv/bin:/usr/local/bin:/usr/bin:/bin"),
                ("VIRTUAL_ENV", "/app/.venv"),
            ]),
            workdir: Some("/app".into()),
            extra_copy: vec![],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FastApiDetector))
}

// ── FastHTML ────────────────────────────────────────────────────────────────

const FAST_HTML: FrameworkId = FrameworkId::new("fasthtml");
inventory::submit! { FrameworkMeta { slug: "fasthtml", display_name: "FastHTML", aliases: &[] } }

pub struct FastHtmlDetector;

impl FrameworkDetector for FastHtmlDetector {
    fn id(&self) -> FrameworkId {
        FAST_HTML
    }
    fn compatible_languages(&self) -> &[LanguageId] {
        &[PYTHON]
    }
    fn detect(&self, deps: &[Dependency]) -> bool {
        deps.iter()
            .any(|d| d.name == "python-fasthtml" || d.name == "fasthtml")
    }
    fn contribution(&self, _deps: &[Dependency]) -> FrameworkContribution {
        FrameworkContribution {
            framework: FAST_HTML,
            default_ports: vec![8000],
            health_endpoints: vec!["/".into()],
            env_vars: BTreeMap::new(),
            runtime_packages: vec![],
            runtime_command: Some(vec![
                "uvicorn".into(),
                "main:app".into(),
                "--host".into(),
                "0.0.0.0".into(),
                "--port".into(),
                "8000".into(),
            ]),
            runtime_env: btree(&[
                ("PATH", "/root/.local/bin:/usr/local/bin:/usr/bin:/bin"),
                ("PYTHONUSERBASE", "/root/.local"),
            ]),
            workdir: Some("/app".into()),
            extra_copy: vec![
                (".".into(), "/app".into()),
                ("/root/.local/".into(), "/root/.local".into()),
            ],
        }
    }
}

inventory::submit! {
    crate::registry::FrameworkDetectorEntry(|| Box::new(FastHtmlDetector))
}
