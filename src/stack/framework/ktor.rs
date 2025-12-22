//! Ktor framework for Kotlin

use super::*;

pub struct KtorFramework;

impl Framework for KtorFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Ktor
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Kotlin".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["gradle".to_string(), "maven".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"io\.ktor:ktor-server-.*".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "io.ktor:ktor-server-core".to_string(),
                confidence: 0.9,
            },
        ]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![8080]
    }

    fn health_endpoints(&self) -> Vec<String> {
        vec!["/health".to_string(), "/healthz".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"KTOR_PORT\s*=\s*(\d+)".to_string(), "Ktor port".to_string()),
            (r"KTOR_ENV\s*=\s*(\w+)".to_string(), "Ktor environment".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_ktor_compatibility() {
        let framework = KtorFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "Kotlin"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "gradle"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "maven"));
    }

    #[test]
    fn test_ktor_dependency_detection() {
        let framework = KtorFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "io.ktor:ktor-server-core".to_string(),
            version: Some("2.3.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_ktor_health_endpoints() {
        let framework = KtorFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/healthz"));
    }

    #[test]
    fn test_ktor_default_ports() {
        let framework = KtorFramework;
        assert_eq!(framework.default_ports(), vec![8080]);
    }
}
