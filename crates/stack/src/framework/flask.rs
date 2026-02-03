//! Flask framework for Python

use super::*;

pub struct FlaskFramework;

impl Framework for FlaskFramework {
    fn id(&self) -> crate::FrameworkId {
        crate::FrameworkId::Flask
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
        // Note: FLASK_APP should be set by build system based on actual file detection
        // See detect_flask_app_file() helper function
        env.insert("FLASK_RUN_HOST".to_string(), "0.0.0.0".to_string());
        env.insert("FLASK_RUN_PORT".to_string(), "5000".to_string());
        env
    }

    fn entrypoint_command(&self) -> Option<Vec<String>> {
        Some(vec!["flask".to_string(), "run".to_string()])
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

/// Detect Flask app file by scanning source files for Flask app instantiation
/// Returns FLASK_APP value in format suitable for the environment variable
///
/// # Arguments
/// * `service_path` - Path to the service directory
/// * `copy_dest` - Destination where files are copied in runtime (e.g., "/app" or "/build")
///
/// # Returns
/// FLASK_APP value like "/app/app.py" or "/build/main.py", or None if not detected
pub fn detect_flask_app_file(service_path: &std::path::Path, copy_dest: &str) -> Option<String> {
    use std::fs;

    // Common Flask app file patterns in priority order
    let candidates = [
        "app.py",
        "main.py",
        "wsgi.py",
        "application.py",
        "run.py",
        "__init__.py",
    ];

    for candidate in &candidates {
        let file_path = service_path.join(candidate);
        if let Ok(content) = fs::read_to_string(&file_path) {
            // Check if this file creates a Flask app instance
            if is_flask_app_file(&content) {
                // Return absolute path in the container
                let filename = file_path.file_name()?.to_str()?;
                return Some(format!("{}/{}", copy_dest, filename));
            }
        }
    }

    // Fallback: check for any .py file with Flask app
    if let Ok(entries) = fs::read_dir(service_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("py") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if is_flask_app_file(&content) {
                        let filename = path.file_name()?.to_str()?;
                        return Some(format!("{}/{}", copy_dest, filename));
                    }
                }
            }
        }
    }

    None
}

/// Check if a Python file contains Flask app instantiation
fn is_flask_app_file(content: &str) -> bool {
    // Look for common Flask app patterns (literal string matching)
    let patterns = [
        "Flask(__name__)",
        "app = Flask(",
        "application = Flask(",
        "def create_app(",
        "app=Flask(", // without spaces
    ];

    patterns.iter().any(|pattern| content.contains(pattern))
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
    use crate::language::Dependency;

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
