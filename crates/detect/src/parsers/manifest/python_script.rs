use crate::helpers::btree;
use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::path::Path;

const PYTHON: LanguageId = LanguageId::new("python");
const PIP: BuildSystemId = BuildSystemId::new("pip");
const PYTHON_RT: RuntimeId = RuntimeId::new("python");

pub struct PythonScriptParser;

impl ManifestParser for PythonScriptParser {
    fn filenames(&self) -> &[&str] {
        &["main.py", "bot.py", "app.py", "server.py", "run.py"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        let parent = path.parent()?;

        // Only produce manifest if no other Python manifest exists in the same directory
        // or parent directory (covers cases like src/main.py with Procfile at root)
        let python_manifests = [
            "pyproject.toml",
            "setup.py",
            "requirements.txt",
            "Pipfile",
            "Pipfile.lock",
            "poetry.lock",
            "setup.cfg",
            "uv.lock",
            "pdm.lock",
            "Procfile",
        ];

        // Check both current directory and parent directory
        let dirs_to_check: Vec<&Path> = {
            let mut dirs = vec![parent];
            if let Some(grandparent) = parent.parent() {
                dirs.push(grandparent);
            }
            dirs
        };

        for dir in &dirs_to_check {
            for manifest_name in &python_manifests {
                let manifest_path = dir.join(manifest_name);
                if manifest_path.exists() {
                    // requirements.txt: only skip if it has content
                    if *manifest_name == "requirements.txt" {
                        if let Ok(req_content) = std::fs::read_to_string(&manifest_path) {
                            if req_content.trim().is_empty() {
                                continue; // Empty requirements.txt, don't skip
                            }
                        }
                    }
                    return None;
                }
            }
        }

        // Verify content looks like Python code
        let has_python_indicators = content.contains("import ")
            || content.contains("from ")
            || content.contains("def ")
            || content.contains("class ")
            || content.contains("print(")
            || content.contains("print ");

        if !has_python_indicators {
            return None;
        }

        let filename = path.file_name()?.to_str()?;

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
                packages: vec!["python".into(), "ca-certificates".into()],
                commands: vec!["echo 'No build step required'".into()],
                member_transform: None,
                env: btree(&[]),
                cache_dirs: vec![],
                artifacts: vec![(".".into(), "/app/".into())],
                build_image: None,
            },
            runtime_config: RuntimeSpec {
                packages: vec!["python".into(), "ca-certificates".into()],
                env: btree(&[]),
                entrypoint: Some(format!("python {}", filename)),
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(PythonScriptParser))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::ManifestParser;

    #[test]
    fn test_python_script_main_py() {
        let parser = PythonScriptParser;
        let content = "print(\"Hello from Python\")";
        // Use a path that won't have real Python manifests next to it
        let manifest = parser
            .parse(Path::new("/tmp/test-python-bare/main.py"), content)
            .unwrap();
        assert_eq!(manifest.language, PYTHON);
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("python main.py".into())
        );
    }

    #[test]
    fn test_python_script_bot_py() {
        let parser = PythonScriptParser;
        let content = "print(\"Hello from bot.py\")";
        let manifest = parser
            .parse(Path::new("/tmp/test-python-bare/bot.py"), content)
            .unwrap();
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("python bot.py".into())
        );
    }

    #[test]
    fn test_python_script_not_python() {
        let parser = PythonScriptParser;
        let content = "Just some random text, not Python code.";
        let result = parser.parse(Path::new("/tmp/test-python-bare/main.py"), content);
        assert!(result.is_none());
    }

    #[test]
    fn test_python2_print_statement() {
        let parser = PythonScriptParser;
        let content = "print \"Hello from Python 2\"";
        let manifest = parser
            .parse(Path::new("/tmp/test-python-bare/main.py"), content)
            .unwrap();
        assert_eq!(manifest.language, PYTHON);
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("python main.py".into())
        );
    }
}
