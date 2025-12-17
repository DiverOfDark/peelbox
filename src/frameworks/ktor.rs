//! Ktor framework for Kotlin

use super::*;

pub struct KtorFramework;

impl Framework for KtorFramework {
    fn name(&self) -> &str {
        "Ktor"
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Kotlin"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["gradle", "maven"]
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

    fn default_ports(&self) -> &[u16] {
        &[8080]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"KTOR_PORT\s*=\s*(\d+)", "Ktor port"),
            (r"KTOR_ENV\s*=\s*(\w+)", "Ktor environment"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_ktor_compatibility() {
        let framework = KtorFramework;

        assert!(framework.compatible_languages().contains(&"Kotlin"));
        assert!(framework.compatible_build_systems().contains(&"gradle"));
        assert!(framework.compatible_build_systems().contains(&"maven"));
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

        assert!(endpoints.contains(&"/health"));
        assert!(endpoints.contains(&"/healthz"));
    }

    #[test]
    fn test_ktor_default_ports() {
        let framework = KtorFramework;
        assert_eq!(framework.default_ports(), &[8080]);
    }
}
