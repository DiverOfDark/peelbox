//! Flask framework for Python

use super::*;

pub struct FlaskFramework;

impl Framework for FlaskFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Flask
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Python".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec![
            "pip".to_string(),
            "poetry".to_string(),
            "pipenv".to_string(),
        ]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::PypiPackage,
                pattern: "flask".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::PypiPackage,
                pattern: "Flask".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![5000]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec!["/health".to_string(), "/healthz".to_string()]
    }

    fn runtime_env_vars(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("FLASK_APP".to_string(), "app:app".to_string());
        env.insert("FLASK_RUN_HOST".to_string(), "0.0.0.0".to_string());
        env.insert("FLASK_RUN_PORT".to_string(), "8080".to_string());
        env
    }

    fn entrypoint_command(&self) -> Option<Vec<String>> {
        Some(vec![
            "python".to_string(),
            "-m".to_string(),
            "flask".to_string(),
            "run".to_string(),
        ])
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r"FLASK_ENV\s*=\s*(\w+)".to_string(),
                "Flask environment".to_string(),
            ),
            (
                r"FLASK_APP\s*=\s*(\S+)".to_string(),
                "Flask application".to_string(),
            ),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec!["config.py", "instance/config.py", "app/config.py"]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("PORT") && trimmed.contains('=') {
                if let Some(eq_pos) = trimmed.find('=') {
                    let value = trimmed[eq_pos + 1..].trim();
                    if let Some(num) = extract_number(value) {
                        config.port = Some(num);
                    }
                }
            }

            if trimmed.contains("os.environ") || trimmed.contains("os.getenv(") {
                extract_env_vars(trimmed, &mut config.env_vars);
            }
        }

        if config.port.is_some() || !config.env_vars.is_empty() {
            Some(config)
        } else {
            None
        }
    }
}

fn extract_number(s: &str) -> Option<u16> {
    let s = s.trim_matches(|c: char| !c.is_numeric());
    s.parse::<u16>().ok()
}

fn extract_env_vars(line: &str, env_vars: &mut Vec<String>) {
    let patterns = ["os.environ.get(", "os.getenv(", "os.environ["];

    for pattern in &patterns {
        let mut pos = 0;
        while let Some(start) = line[pos..].find(pattern) {
            let abs_start = pos + start + pattern.len();
            let rest = &line[abs_start..];

            if let Some(quote_start) = rest.find(['"', '\'']) {
                let quote_char = rest.chars().nth(quote_start).unwrap();
                let after_quote = &rest[quote_start + 1..];

                if let Some(quote_end) = after_quote.find(quote_char) {
                    let var_name = &after_quote[..quote_end];
                    if !var_name.is_empty() && !env_vars.contains(&var_name.to_string()) {
                        env_vars.push(var_name.to_string());
                    }
                }
            }

            pos = abs_start + 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_flask_compatibility() {
        let framework = FlaskFramework;

        assert!(framework
            .compatible_languages()
            .iter()
            .any(|s| s == "Python"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "pip"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "poetry"));
    }

    #[test]
    fn test_flask_dependency_detection() {
        let framework = FlaskFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "flask".to_string(),
            version: Some("3.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_flask_health_endpoints() {
        let framework = FlaskFramework;
        let endpoints = framework.health_endpoints(&[]);

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/healthz"));
    }

    #[test]
    fn test_flask_default_ports() {
        let framework = FlaskFramework;
        assert_eq!(framework.default_ports(), vec![5000]);
    }

    #[test]
    fn test_flask_parse_config() {
        let framework = FlaskFramework;
        let content = r#"
import os

PORT = int(os.environ.get('PORT', 3000))
SECRET_KEY = os.getenv('SECRET_KEY')
DATABASE_URL = os.environ['DB_URL']
"#;

        let config = framework
            .parse_config(Path::new("config.py"), content)
            .unwrap();

        assert_eq!(config.port, Some(3000));
        assert!(config.env_vars.contains(&"PORT".to_string()));
        assert!(config.env_vars.contains(&"SECRET_KEY".to_string()));
        assert!(config.env_vars.contains(&"DB_URL".to_string()));
    }

    #[test]
    fn test_flask_config_files() {
        let framework = FlaskFramework;
        let files = framework.config_files();

        assert!(files.contains(&"config.py"));
        assert!(files.contains(&"instance/config.py"));
    }
}
