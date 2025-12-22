//! Python language definition (pip, poetry, pipenv)

use super::{
    parsers::{DependencyParser, RegexDependencyParser},
    Dependency, DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition,
};
use regex::Regex;
use std::collections::HashSet;

pub struct PythonLanguage;

impl LanguageDefinition for PythonLanguage {
    fn id(&self) -> crate::stack::LanguageId {
        crate::stack::LanguageId::Python
    }

    fn extensions(&self) -> Vec<String> {
        vec!["py".to_string(), "pyi".to_string(), "pyw".to_string()]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "pyproject.toml" => {
                let mut build_system = crate::stack::BuildSystemId::Pip;
                let mut confidence = 0.85;

                if let Some(content) = manifest_content {
                    if content.contains("[tool.poetry]") {
                        build_system = crate::stack::BuildSystemId::Poetry;
                        confidence = 1.0;
                    } else if content.contains("[project]") {
                        confidence = 0.9;
                    }
                }

                Some(DetectionResult {
                    build_system,
                    confidence,
                })
            }
            "Pipfile" => Some(DetectionResult {
                build_system: crate::stack::BuildSystemId::Pipenv,
                confidence: 1.0,
            }),
            "requirements.txt" => {
                let mut confidence = 0.9;
                if let Some(content) = manifest_content {
                    if content
                        .lines()
                        .any(|l| !l.trim().is_empty() && !l.starts_with('#'))
                    {
                        confidence = 1.0;
                    }
                }
                Some(DetectionResult {
                    build_system: crate::stack::BuildSystemId::Pip,
                    confidence,
                })
            }
            "setup.py" | "setup.cfg" => Some(DetectionResult {
                build_system: crate::stack::BuildSystemId::Pip,
                confidence: 0.85,
            }),
            _ => None,
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["pip".to_string(), "poetry".to_string(), "pipenv".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec![
            "__pycache__".to_string(),
            ".venv".to_string(),
            "venv".to_string(),
            ".tox".to_string(),
            ".pytest_cache".to_string(),
            ".mypy_cache".to_string(),
            "dist".to_string(),
            "build".to_string(),
            "*.egg-info".to_string(),
        ]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec![]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;

        // pyproject.toml: requires-python = ">=3.11"
        if let Some(caps) = Regex::new(r#"requires-python\s*=\s*"[^"]*(\d+\.\d+)"#)
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // Pipfile: python_version = "3.11"
        if let Some(caps) = Regex::new(r#"python_version\s*=\s*"(\d+\.\d+)""#)
            .ok()
            .and_then(|re| re.captures(content))
        {
            return Some(caps.get(1)?.as_str().to_string());
        }

        // .python-version file (just contains version)
        if !content.contains('[') && !content.contains('{') {
            let trimmed = content.trim();
            if Regex::new(r"^\d+\.\d+").ok()?.is_match(trimmed) {
                if let Some(caps) = Regex::new(r"^(\d+\.\d+)").ok()?.captures(trimmed) {
                    return Some(caps.get(1)?.as_str().to_string());
                }
            }
        }

        None
    }

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        _all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        if manifest_content.contains("[tool.poetry") {
            self.parse_poetry_dependencies(manifest_content)
        } else if !manifest_content.contains('[') {
            self.parse_requirements_txt(manifest_content)
        } else {
            DependencyInfo::empty()
        }
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r#"os\.environ\.get\(['"]([A-Z_][A-Z0-9_]*)['"]"#.to_string(), "os.environ.get".to_string()),
            (r#"os\.getenv\(['"]([A-Z_][A-Z0-9_]*)['"]"#.to_string(), "os.getenv".to_string()),
        ]
    }

    fn port_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"@app\.route.*:(\d{4,5})".to_string(), "Flask route decorator".to_string()),
            (r"app\.run\(.*port\s*=\s*(\d{4,5})".to_string(), "app.run()".to_string()),
            (r"uvicorn\.run\(.*port\s*=\s*(\d{4,5})".to_string(), "uvicorn.run()".to_string()),
        ]
    }

    fn runtime_name(&self) -> Option<String> {
        Some("python".to_string())
    }

    fn default_port(&self) -> Option<u16> {
        Some(8000)
    }

    fn is_main_file(&self, _fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            ext == "py" || ext == "pyw" || ext == "pyi"
        } else {
            false
        }
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("python main.py".to_string())
    }

    fn parse_entrypoint_from_manifest(&self, _manifest_content: &str) -> Option<String> {
        None
    }
}

impl PythonLanguage {
    fn parse_poetry_dependencies(&self, content: &str) -> DependencyInfo {
        let parsed: toml::Value = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return DependencyInfo::empty(),
        };

        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        if let Some(tool) = parsed.get("tool").and_then(|t| t.as_table()) {
            if let Some(poetry) = tool.get("poetry").and_then(|p| p.as_table()) {
                for dep_section in vec!["dependencies".to_string(), "dev-dependencies".to_string()] {
                    if let Some(deps) = poetry.get(&dep_section).and_then(|d| d.as_table()) {
                        for (name, value) in deps {
                            if name == "python" || seen.contains(name) {
                                continue;
                            }
                            seen.insert(name.clone());

                            let version = if let Some(ver) = value.as_str() {
                                Some(ver.to_string())
                            } else if let Some(table) = value.as_table() {
                                table
                                    .get("version")
                                    .and_then(|v| v.as_str())
                                    .map(String::from)
                            } else {
                                None
                            };

                            external_deps.push(Dependency {
                                name: name.clone(),
                                version,
                                is_internal: false,
                            });
                        }
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps: vec![],
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn parse_requirements_txt(&self, content: &str) -> DependencyInfo {
        let dep_re = Regex::new(r"^([a-zA-Z0-9_-]+)(?:==|>=|<=|~=|!=)?([^\s#]*)").unwrap();
        RegexDependencyParser {
            line_pattern: dep_re,
            internal_check: |_name, _paths| false,
        }
        .parse(content, &[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions() {
        let lang = PythonLanguage;
        assert!(lang.extensions().iter().any(|s| s == "py"));
    }

    #[test]
    fn test_detect_requirements() {
        let lang = PythonLanguage;
        let result = lang.detect("requirements.txt", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::Pip);
    }

    #[test]
    fn test_detect_pipfile() {
        let lang = PythonLanguage;
        let result = lang.detect("Pipfile", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::Pipenv);
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_pyproject_poetry() {
        let lang = PythonLanguage;
        let content = r#"
[tool.poetry]
name = "myapp"
version = "0.1.0"
"#;
        let result = lang.detect("pyproject.toml", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::Poetry);
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_pyproject_pep621() {
        let lang = PythonLanguage;
        let content = r#"
[project]
name = "myapp"
version = "0.1.0"
"#;
        let result = lang.detect("pyproject.toml", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::stack::BuildSystemId::Pip);
    }

    #[test]
    fn test_compatible_build_systems() {
        let lang = PythonLanguage;
        let systems = lang.compatible_build_systems();
        assert!(systems.iter().any(|s| s == "pip"));
        assert!(systems.iter().any(|s| s == "poetry"));
        assert!(systems.iter().any(|s| s == "pipenv"));
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = PythonLanguage;
        assert!(lang.excluded_dirs().iter().any(|s| s == "__pycache__"));
        assert!(lang.excluded_dirs().iter().any(|s| s == ".venv"));
    }

    #[test]
    fn test_detect_version_pyproject() {
        let lang = PythonLanguage;
        let content = r#"[project]
requires-python = ">=3.11"
"#;
        assert_eq!(lang.detect_version(Some(content)), Some("3.11".to_string()));
    }

    #[test]
    fn test_detect_version_pipfile() {
        let lang = PythonLanguage;
        let content = r#"[requires]
python_version = "3.10"
"#;
        assert_eq!(lang.detect_version(Some(content)), Some("3.10".to_string()));
    }

    #[test]
    fn test_detect_version_file() {
        let lang = PythonLanguage;
        assert_eq!(
            lang.detect_version(Some("3.11.4")),
            Some("3.11".to_string())
        );
    }

    #[test]
    fn test_parse_dependencies_requirements_txt() {
        let lang = PythonLanguage;
        let content = r#"
flask==2.3.0
requests>=2.28.0
pytest
# comment
django==4.2.0
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 4);
        assert!(deps.external_deps.iter().any(|d| d.name == "flask"));
        assert!(deps.external_deps.iter().any(|d| d.name == "requests"));
        assert!(deps.external_deps.iter().any(|d| d.name == "pytest"));
    }

    #[test]
    fn test_parse_dependencies_poetry() {
        let lang = PythonLanguage;
        let content = r#"
[tool.poetry]
name = "myapp"

[tool.poetry.dependencies]
python = "^3.11"
flask = "^2.3.0"
requests = { version = "^2.28.0", extras = ["security"] }

[tool.poetry.dev-dependencies]
pytest = "^7.4.0"
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 3);
        assert!(deps.external_deps.iter().any(|d| d.name == "flask"));
        assert!(deps.external_deps.iter().any(|d| d.name == "requests"));
        assert!(deps.external_deps.iter().any(|d| d.name == "pytest"));
    }

    #[test]
    fn test_parse_dependencies_poetry_skips_python() {
        let lang = PythonLanguage;
        let content = r#"
[tool.poetry.dependencies]
python = "^3.11"
flask = "^2.3.0"
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 1);
        assert!(deps.external_deps.iter().all(|d| d.name != "python"));
    }
}
