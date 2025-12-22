//! Actix Web framework for Rust

use super::*;

pub struct ActixFramework;

impl Framework for ActixFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::ActixWeb
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Rust".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["cargo".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"actix-web".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> &[u16] {
        &[8080]
    }

    fn health_endpoints(&self) -> Vec<String> {
        vec!["/health".to_string(), "/healthz".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"ACTIX_HOST\s*=\s*(\S+)".to_string(), "Actix host".to_string()),
            (r"ACTIX_PORT\s*=\s*(\d+)".to_string(), "Actix port".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_actix_compatibility() {
        let framework = ActixFramework;
        assert!(framework.compatible_languages().iter().any(|s| s == "Rust"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "cargo"));
    }

    #[test]
    fn test_actix_dependency_detection() {
        let framework = ActixFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "actix-web".to_string(),
            version: Some("4.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_actix_default_ports() {
        let framework = ActixFramework;
        assert_eq!(framework.default_ports(), &[8080]);
    }
}
