//! Quarkus framework for Java/Kotlin

use super::*;

pub struct QuarkusFramework;

impl Framework for QuarkusFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Quarkus
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

    fn default_ports(&self) -> Vec<u16> {
        vec![8080]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec![
            "/q/health".to_string(),
            "/q/health/live".to_string(),
            "/q/health/ready".to_string(),
        ]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r"QUARKUS_HTTP_PORT\s*=\s*(\d+)".to_string(),
                "Quarkus HTTP port".to_string(),
            ),
            (
                r"QUARKUS_PROFILE\s*=\s*(\w+)".to_string(),
                "Quarkus profile".to_string(),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_quarkus_compatibility() {
        let framework = QuarkusFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "Java"));
        assert!(framework
            .compatible_languages()
            .iter()
            .any(|s| s == "Kotlin"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "maven"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "gradle"));
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
        let endpoints = framework.health_endpoints(&[]);

        assert!(endpoints.iter().any(|s| s == "/q/health"));
        assert!(endpoints.iter().any(|s| s == "/q/health/live"));
    }

    #[test]
    fn test_quarkus_default_ports() {
        let framework = QuarkusFramework;
        assert_eq!(framework.default_ports(), vec![8080]);
    }
}
