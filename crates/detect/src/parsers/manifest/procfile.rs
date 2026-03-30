use crate::helpers::btree;
use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const PYTHON: LanguageId = LanguageId::new("python");
const PIP: BuildSystemId = BuildSystemId::new("pip");
const PYTHON_RT: RuntimeId = RuntimeId::new("python");

pub struct ProcfileParser;

impl ManifestParser for ProcfileParser {
    fn filenames(&self) -> &[&str] {
        &["Procfile"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        // Find the web: line
        let web_line = content.lines().find(|l| l.trim().starts_with("web:"))?;

        let command = web_line.trim().strip_prefix("web:")?.trim().to_string();

        if command.is_empty() {
            return None;
        }

        // Only handle Python-based Procfiles
        let is_python = command.starts_with("python")
            || command.starts_with("gunicorn")
            || command.starts_with("uvicorn");

        if !is_python {
            return None;
        }

        // Check if requirements.txt exists and has content in the same directory
        let parent = path.parent()?;
        let has_requirements = parent
            .join("requirements.txt")
            .metadata()
            .ok()
            .map(|m| m.len() > 0)
            .unwrap_or(false);

        let build_commands = if has_requirements {
            vec!["pip install --user --no-cache-dir -r requirements.txt".into()]
        } else {
            // Even with no requirements, we need at least one build command
            // for validation to pass
            vec!["true".into()]
        };

        let mut build_packages = vec!["python".into(), "ca-certificates".into()];
        if has_requirements {
            build_packages.insert(1, "pip".into());
            build_packages.insert(2, "build-base".into());
        }

        let mut artifacts = vec![(".".into(), "/app/".into())];
        if has_requirements {
            artifacts.push(("/root/.local/".into(), "/root/.local/".into()));
        }

        let runtime_env = if has_requirements {
            btree(&[
                ("PATH", "/root/.local/bin:/usr/local/bin:/usr/bin:/bin"),
                ("PYTHONUSERBASE", "/root/.local"),
            ])
        } else {
            btree(&[])
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language: PYTHON,
            build_system: PIP,
            runtime: PYTHON_RT,
            package: Some(Package {
                name: "app".to_string(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages: build_packages,
                commands: build_commands,
                member_transform: None,
                env: btree(&[]),
                cache_dirs: vec![".cache/pip".into()],
                artifacts,
                setup_commands: vec![],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec![
                    "python".into(),
                    "libgcc".into(),
                    "libstdc++".into(),
                    "ca-certificates".into(),
                ],
                env: runtime_env,
                entrypoint: Some(command),
                workdir: Some("/app".into()),
                ports: vec![8000],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(ProcfileParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_procfile_python_command() {
        let parser = ProcfileParser;
        let content = "web: python src/main.py\nworker: echo \"another process\"";
        let manifest = parser.parse(Path::new("/tmp/Procfile"), content).unwrap();
        assert_eq!(manifest.language, PYTHON);
        assert_eq!(manifest.build_system, PIP);
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("python src/main.py".into())
        );
    }

    #[test]
    fn test_procfile_gunicorn_command() {
        let parser = ProcfileParser;
        let content = "web: gunicorn app:app --bind 0.0.0.0:8000";
        let manifest = parser.parse(Path::new("/tmp/Procfile"), content).unwrap();
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("gunicorn app:app --bind 0.0.0.0:8000".into())
        );
    }

    #[test]
    fn test_procfile_uvicorn_command() {
        let parser = ProcfileParser;
        let content = "web: uvicorn main:app --host 0.0.0.0 --port 8000";
        let manifest = parser.parse(Path::new("/tmp/Procfile"), content).unwrap();
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("uvicorn main:app --host 0.0.0.0 --port 8000".into())
        );
    }

    #[test]
    fn test_procfile_non_python_returns_none() {
        let parser = ProcfileParser;
        let content = "web: node server.js";
        let result = parser.parse(Path::new("/tmp/Procfile"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_procfile_no_web_returns_none() {
        let parser = ProcfileParser;
        let content = "worker: python worker.py";
        let result = parser.parse(Path::new("/tmp/Procfile"), content);
        assert!(result.is_none());
    }
}
