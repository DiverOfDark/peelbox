//! Django framework for Python

use super::*;

pub struct DjangoFramework;

impl Framework for DjangoFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Django
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

    fn default_ports(&self) -> &[u16] {
        &[8000]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health/", "/healthz/", "/ping/"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                r"DJANGO_SETTINGS_MODULE\s*=\s*(\S+)",
                "Django settings module",
            ),
            (r"SECRET_KEY\s*=\s*", "Django secret key"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_django_compatibility() {
        let framework = DjangoFramework;

        assert!(framework.compatible_languages().contains(&"Python"));
        assert!(framework.compatible_build_systems().contains(&"pip"));
        assert!(framework.compatible_build_systems().contains(&"poetry"));
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
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/health/"));
        assert!(endpoints.contains(&"/healthz/"));
    }

    #[test]
    fn test_django_default_ports() {
        let framework = DjangoFramework;
        assert_eq!(framework.default_ports(), &[8000]);
    }
}
