//! Echo framework for Go

use super::*;

pub struct EchoFramework;

impl Framework for EchoFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Echo
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Go"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["go"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"github\.com/labstack/echo".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> &[u16] {
        &[1323]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![(r"PORT\s*=\s*(\d+)", "Server port")]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_echo_compatibility() {
        let framework = EchoFramework;
        assert!(framework.compatible_languages().contains(&"Go"));
        assert!(framework.compatible_build_systems().contains(&"go"));
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
        assert_eq!(framework.default_ports(), &[1323]);
    }
}
