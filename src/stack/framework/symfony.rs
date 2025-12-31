//! Symfony framework for PHP

use super::*;

pub struct SymfonyFramework;

impl Framework for SymfonyFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Symfony
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["PHP".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["composer".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"symfony/framework-bundle".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"symfony/http-kernel".to_string(),
                confidence: 0.90,
            },
        ]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![8000]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec!["/_health".to_string(), "/health".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r"%env\(([A-Z_]+)\)%".to_string(),
                "Symfony environment variable".to_string(),
            ),
            (
                r"APP_ENV\s*=\s*(\w+)".to_string(),
                "Symfony environment".to_string(),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_symfony_compatibility() {
        let framework = SymfonyFramework;
        assert!(framework.compatible_languages().iter().any(|s| s == "PHP"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "composer"));
    }

    #[test]
    fn test_symfony_dependency_detection() {
        let framework = SymfonyFramework;
        let patterns = framework.dependency_patterns();

        let dep1 = Dependency {
            name: "symfony/framework-bundle".to_string(),
            version: Some("6.4.0".to_string()),
            is_internal: false,
        };

        let dep2 = Dependency {
            name: "symfony/http-kernel".to_string(),
            version: Some("6.4.0".to_string()),
            is_internal: false,
        };

        let matches1: Vec<_> = patterns.iter().filter(|p| p.matches(&dep1)).collect();
        assert!(!matches1.is_empty());
        assert!(matches1[0].confidence >= 0.9);

        let matches2: Vec<_> = patterns.iter().filter(|p| p.matches(&dep2)).collect();
        assert!(!matches2.is_empty());
        assert!(matches2[0].confidence >= 0.9);
    }

    #[test]
    fn test_symfony_default_ports() {
        let framework = SymfonyFramework;
        assert_eq!(framework.default_ports(), vec![8000]);
    }

    #[test]
    fn test_symfony_health_endpoints() {
        let framework = SymfonyFramework;
        let endpoints = framework.health_endpoints(&[]);
        assert!(endpoints.iter().any(|s| s == "/_health"));
        assert!(endpoints.iter().any(|s| s == "/health"));
    }

    #[test]
    fn test_symfony_env_var_patterns() {
        let framework = SymfonyFramework;
        let patterns = framework.env_var_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.iter().any(|(p, _)| p.contains(r"%env\(")));
    }
}
