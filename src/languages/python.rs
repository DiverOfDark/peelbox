//! Python language definition (pip, poetry, pipenv)

use super::{
    BuildTemplate, Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition, ManifestPattern,
};
use regex::Regex;
use std::collections::HashSet;

pub struct PythonLanguage;

impl LanguageDefinition for PythonLanguage {
    fn name(&self) -> &str {
        "Python"
    }

    fn extensions(&self) -> &[&str] {
        &["py", "pyi", "pyw"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[
            ManifestPattern {
                filename: "pyproject.toml",
                build_system: "poetry",
                priority: 12,
            },
            ManifestPattern {
                filename: "Pipfile",
                build_system: "pipenv",
                priority: 10,
            },
            ManifestPattern {
                filename: "requirements.txt",
                build_system: "pip",
                priority: 8,
            },
            ManifestPattern {
                filename: "setup.py",
                build_system: "pip",
                priority: 6,
            },
            ManifestPattern {
                filename: "setup.cfg",
                build_system: "pip",
                priority: 5,
            },
        ]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "pyproject.toml" => {
                let mut build_system = "pip".to_string();
                let mut confidence = 0.85;

                if let Some(content) = manifest_content {
                    if content.contains("[tool.poetry]") {
                        build_system = "poetry".to_string();
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
                build_system: "pipenv".to_string(),
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
                    build_system: "pip".to_string(),
                    confidence,
                })
            }
            "setup.py" | "setup.cfg" => Some(DetectionResult {
                build_system: "pip".to_string(),
                confidence: 0.85,
            }),
            _ => None,
        }
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        match build_system {
            "pip" => Some(BuildTemplate {
                build_image: "python:3.11".to_string(),
                runtime_image: "python:3.11-slim".to_string(),
                build_packages: vec!["build-essential".to_string()],
                runtime_packages: vec![],
                build_commands: vec!["pip install --no-cache-dir -r requirements.txt".to_string()],
                cache_paths: vec!["/root/.cache/pip/".to_string()],
                artifacts: vec![
                    "/usr/local/lib/python3.11/site-packages".to_string(),
                    "app/".to_string(),
                ],
                common_ports: vec![8000, 5000],
            }),
            "poetry" => Some(BuildTemplate {
                build_image: "python:3.11".to_string(),
                runtime_image: "python:3.11-slim".to_string(),
                build_packages: vec!["build-essential".to_string()],
                runtime_packages: vec![],
                build_commands: vec![
                    "pip install poetry".to_string(),
                    "poetry install --no-dev".to_string(),
                ],
                cache_paths: vec![".venv/".to_string(), "/root/.cache/pypoetry/".to_string()],
                artifacts: vec!["dist/".to_string(), ".venv/".to_string()],
                common_ports: vec![8000, 5000],
            }),
            "pipenv" => Some(BuildTemplate {
                build_image: "python:3.11".to_string(),
                runtime_image: "python:3.11-slim".to_string(),
                build_packages: vec!["build-essential".to_string()],
                runtime_packages: vec![],
                build_commands: vec![
                    "pip install pipenv".to_string(),
                    "pipenv install --deploy".to_string(),
                ],
                cache_paths: vec![
                    "/root/.cache/pip/".to_string(),
                    "/root/.cache/pipenv/".to_string(),
                ],
                artifacts: vec!["Pipfile".to_string()],
                common_ports: vec![8000, 5000],
            }),
            _ => None,
        }
    }

    fn build_systems(&self) -> &[&str] {
        &["pip", "poetry", "pipenv"]
    }

    fn excluded_dirs(&self) -> &[&str] {
        &[
            "__pycache__",
            ".venv",
            "venv",
            ".tox",
            ".pytest_cache",
            ".mypy_cache",
            "dist",
            "build",
            "*.egg-info",
        ]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[]
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
                for dep_section in &["dependencies", "dev-dependencies"] {
                    if let Some(deps) = poetry.get(*dep_section).and_then(|d| d.as_table()) {
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
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        let dep_re = Regex::new(r"^([a-zA-Z0-9_-]+)(?:==|>=|<=|~=|!=)?([^\s#]*)").ok();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }

            if let Some(ref re) = dep_re {
                if let Some(caps) = re.captures(trimmed) {
                    if let Some(name) = caps.get(1) {
                        let name_str = name.as_str().to_string();
                        if seen.contains(&name_str) {
                            continue;
                        }
                        seen.insert(name_str.clone());

                        let version = caps.get(2).and_then(|v| {
                            let s = v.as_str().trim();
                            if s.is_empty() {
                                None
                            } else {
                                Some(s.to_string())
                            }
                        });

                        external_deps.push(Dependency {
                            name: name_str,
                            version,
                            is_internal: false,
                        });
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = PythonLanguage;
        assert_eq!(lang.name(), "Python");
    }

    #[test]
    fn test_extensions() {
        let lang = PythonLanguage;
        assert!(lang.extensions().contains(&"py"));
    }

    #[test]
    fn test_detect_requirements() {
        let lang = PythonLanguage;
        let result = lang.detect("requirements.txt", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "pip");
    }

    #[test]
    fn test_detect_pipfile() {
        let lang = PythonLanguage;
        let result = lang.detect("Pipfile", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "pipenv");
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
        assert_eq!(r.build_system, "poetry");
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
        assert_eq!(r.build_system, "pip");
    }

    #[test]
    fn test_build_template_pip() {
        let lang = PythonLanguage;
        let template = lang.build_template("pip");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_image.contains("python"));
        assert!(t.build_commands.iter().any(|c| c.contains("pip")));
    }

    #[test]
    fn test_build_template_poetry() {
        let lang = PythonLanguage;
        let template = lang.build_template("poetry");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_commands.iter().any(|c| c.contains("poetry")));
    }

    #[test]
    fn test_build_template_pipenv() {
        let lang = PythonLanguage;
        let template = lang.build_template("pipenv");
        assert!(template.is_some());
        let t = template.unwrap();
        assert!(t.build_commands.iter().any(|c| c.contains("pipenv")));
    }

    #[test]
    fn test_excluded_dirs() {
        let lang = PythonLanguage;
        assert!(lang.excluded_dirs().contains(&"__pycache__"));
        assert!(lang.excluded_dirs().contains(&".venv"));
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
