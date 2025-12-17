//! Quarkus framework for Java/Kotlin

use super::*;

pub struct QuarkusFramework;

impl Framework for QuarkusFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Quarkus
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
                pattern: r"io\.quarkus:quarkus-.*".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "io.quarkus:quarkus-resteasy".to_string(),
                confidence: 0.9,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[8080]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/q/health", "/q/health/live", "/q/health/ready"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"QUARKUS_HTTP_PORT\s*=\s*(\d+)", "Quarkus HTTP port"),
            (r"QUARKUS_PROFILE\s*=\s*(\w+)", "Quarkus profile"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_quarkus_compatibility() {
        let framework = QuarkusFramework;

        assert!(framework.compatible_languages().contains(&"Java"));
        assert!(framework.compatible_languages().contains(&"Kotlin"));
        assert!(framework.compatible_build_systems().contains(&"maven"));
        assert!(framework.compatible_build_systems().contains(&"gradle"));
    }

    #[test]
    fn test_quarkus_dependency_detection() {
        let framework = QuarkusFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "io.quarkus:quarkus-resteasy".to_string(),
            version: Some("3.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_quarkus_health_endpoints() {
        let framework = QuarkusFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/q/health"));
        assert!(endpoints.contains(&"/q/health/live"));
    }

    #[test]
    fn test_quarkus_default_ports() {
        let framework = QuarkusFramework;
        assert_eq!(framework.default_ports(), &[8080]);
    }
}
