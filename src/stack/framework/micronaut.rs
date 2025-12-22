//! Micronaut framework for Java/Kotlin

use super::*;

pub struct MicronautFramework;

impl Framework for MicronautFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Micronaut
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Java".to_string(), "Kotlin".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["maven".to_string(), "gradle".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"io\.micronaut:micronaut-.*".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "io.micronaut:micronaut-http".to_string(),
                confidence: 0.9,
            },
        ]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![8080]
    }

    fn health_endpoints(&self) -> Vec<String> {
        vec!["/health".to_string(), "/health/liveness".to_string(), "/health/readiness".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r"MICRONAUT_SERVER_PORT\s*=\s*(\d+)".to_string(),
                "Micronaut server port".to_string(),
            ),
            (
                r"MICRONAUT_ENVIRONMENTS\s*=\s*(\w+)".to_string(),
                "Micronaut environments".to_string(),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_micronaut_compatibility() {
        let framework = MicronautFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "Java"));
        assert!(framework.compatible_languages().iter().any(|s| s == "Kotlin"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "maven"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "gradle"));
    }

    #[test]
    fn test_micronaut_dependency_detection() {
        let framework = MicronautFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "io.micronaut:micronaut-http".to_string(),
            version: Some("4.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_micronaut_health_endpoints() {
        let framework = MicronautFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/health/liveness"));
    }

    #[test]
    fn test_micronaut_default_ports() {
        let framework = MicronautFramework;
        assert_eq!(framework.default_ports(), vec![8080]);
    }
}
