//! Echo framework for Go

use super::*;

pub struct EchoFramework;

impl Framework for EchoFramework {
    fn id(&self) -> crate::FrameworkId {
        crate::FrameworkId::Echo
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Go".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["go".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"github\.com/labstack/echo".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![1323]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec!["/health".to_string(), "/healthz".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![(r"PORT\s*=\s*(\d+)".to_string(), "Server port".to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::Dependency;

    #[test]
    fn test_echo_compatibility() {
        let framework = EchoFramework;
        assert!(framework.compatible_languages().iter().any(|s| s == "Go"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "go"));
    }

    #[test]
    fn test_echo_dependency_detection() {
        let framework = EchoFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "github.com/labstack/echo".to_string(),
            version: Some("v4.11.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_echo_default_ports() {
        let framework = EchoFramework;
        assert_eq!(framework.default_ports(), vec![1323]);
    }
}
