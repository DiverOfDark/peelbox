//! Flask framework for Python

use super::*;

pub struct FlaskFramework;

impl Framework for FlaskFramework {
    fn name(&self) -> &str {
        "Flask"
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Python"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["pip", "poetry", "pipenv"]
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

    fn default_ports(&self) -> &[u16] {
        &[5000]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"FLASK_ENV\s*=\s*(\w+)", "Flask environment"),
            (r"FLASK_APP\s*=\s*(\S+)", "Flask application"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_flask_compatibility() {
        let framework = FlaskFramework;

        assert!(framework.compatible_languages().contains(&"Python"));
        assert!(framework.compatible_build_systems().contains(&"pip"));
        assert!(framework.compatible_build_systems().contains(&"poetry"));
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
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/health"));
        assert!(endpoints.contains(&"/healthz"));
    }

    #[test]
    fn test_flask_default_ports() {
        let framework = FlaskFramework;
        assert_eq!(framework.default_ports(), &[5000]);
    }
}
