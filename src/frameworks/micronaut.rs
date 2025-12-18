//! Micronaut framework for Java/Kotlin

use super::*;

pub struct MicronautFramework;

impl Framework for MicronautFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Micronaut
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Java", "Kotlin"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["maven", "gradle"]
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

    fn default_ports(&self) -> &[u16] {
        &[8080]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/health/liveness", "/health/readiness"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                r"MICRONAUT_SERVER_PORT\s*=\s*(\d+)",
                "Micronaut server port",
            ),
            (
                r"MICRONAUT_ENVIRONMENTS\s*=\s*(\w+)",
                "Micronaut environments",
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_micronaut_compatibility() {
        let framework = MicronautFramework;

        assert!(framework.compatible_languages().contains(&"Java"));
        assert!(framework.compatible_languages().contains(&"Kotlin"));
        assert!(framework.compatible_build_systems().contains(&"maven"));
        assert!(framework.compatible_build_systems().contains(&"gradle"));
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

        assert!(endpoints.contains(&"/health"));
        assert!(endpoints.contains(&"/health/liveness"));
    }

    #[test]
    fn test_micronaut_default_ports() {
        let framework = MicronautFramework;
        assert_eq!(framework.default_ports(), &[8080]);
    }
}
