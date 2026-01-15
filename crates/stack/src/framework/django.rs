//! Django framework for Python

use super::*;

pub struct DjangoFramework;

impl Framework for DjangoFramework {
    fn id(&self) -> crate::FrameworkId {
        crate::FrameworkId::Django
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
                pattern: "django".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::PypiPackage,
                pattern: "Django".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![8000]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec![
            "/health/".to_string(),
            "/healthz/".to_string(),
            "/ping/".to_string(),
        ]
    }

    fn entrypoint_command(&self) -> Option<Vec<String>> {
        Some(vec![
            "python".to_string(),
            "manage.py".to_string(),
            "runserver".to_string(),
            "0.0.0.0:8080".to_string(),
        ])
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r"DJANGO_SETTINGS_MODULE\s*=\s*(\S+)".to_string(),
                "Django settings module".to_string(),
            ),
            (
                r"SECRET_KEY\s*=\s*".to_string(),
                "Django secret key".to_string(),
            ),
        ]
    }

    fn config_files(&self) -> Vec<&str> {
        vec!["settings.py", "*/settings.py", "config/settings.py"]
    }

    fn parse_config(&self, _file_path: &Path, content: &str) -> Option<FrameworkConfig> {
        let mut config = FrameworkConfig::default();

        for line in content.lines() {
            let trimmed = line.trim();

            if let Some(port_str) = trimmed.strip_prefix("PORT") {
                if let Some(eq_pos) = port_str.find('=') {
                    let value = port_str[eq_pos + 1..].trim();
                    if let Some(num) = extract_number(value) {
                        config.port = Some(num);
                    }
                }
            }

            if trimmed.contains("os.environ") || trimmed.contains("os.getenv(") {
                extract_django_env_vars(trimmed, &mut config.env_vars);
            }

            if trimmed.contains("ALLOWED_HOSTS") {
                if let Some(eq_pos) = trimmed.find('=') {
                    let value = trimmed[eq_pos + 1..].trim();
                    if value.contains("os.environ") {
                        extract_django_env_vars(value, &mut config.env_vars);
                    }
                }
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

fn extract_django_env_vars(line: &str, env_vars: &mut Vec<String>) {
    let patterns = [
        ("os.environ.get(", true),
        ("os.getenv(", true),
        ("environ.get(", true),
        ("getenv(", true),
        ("os.environ[", false),
    ];

    for (pattern, _uses_parens) in &patterns {
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
    fn test_django_compatibility() {
        let framework = DjangoFramework;

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
    fn test_django_dependency_detection() {
        let framework = DjangoFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "django".to_string(),
            version: Some("4.2.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_django_health_endpoints() {
        let framework = DjangoFramework;
        let endpoints = framework.health_endpoints(&[]);

        assert!(endpoints.iter().any(|s| s == "/health/"));
        assert!(endpoints.iter().any(|s| s == "/healthz/"));
    }

    #[test]
    fn test_django_default_ports() {
        let framework = DjangoFramework;
        assert_eq!(framework.default_ports(), vec![8000]);
    }

    #[test]
    fn test_django_parse_settings() {
        let framework = DjangoFramework;
        let content = r#"
import os

PORT = int(os.environ.get('PORT', 8080))
SECRET_KEY = os.getenv('SECRET_KEY')
ALLOWED_HOSTS = [os.environ.get('ALLOWED_HOST', 'localhost')]

DATABASE_URL = os.environ['DATABASE_URL']
"#;

        let config = framework
            .parse_config(Path::new("settings.py"), content)
            .unwrap();

        assert_eq!(config.port, Some(8080));
        assert!(config.env_vars.contains(&"PORT".to_string()));
        assert!(config.env_vars.contains(&"SECRET_KEY".to_string()));
        assert!(config.env_vars.contains(&"ALLOWED_HOST".to_string()));
        assert!(config.env_vars.contains(&"DATABASE_URL".to_string()));
    }

    #[test]
    fn test_django_config_files() {
        let framework = DjangoFramework;
        let files = framework.config_files();

        assert!(files.contains(&"settings.py"));
        assert!(files.contains(&"*/settings.py"));
    }
}
