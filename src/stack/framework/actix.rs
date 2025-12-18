//! Actix Web framework for Rust

use super::*;

pub struct ActixFramework;

impl Framework for ActixFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::ActixWeb
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Rust"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["cargo"]
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

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"ACTIX_HOST\s*=\s*(\S+)", "Actix host"),
            (r"ACTIX_PORT\s*=\s*(\d+)", "Actix port"),
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
        assert!(framework.compatible_languages().contains(&"Rust"));
        assert!(framework.compatible_build_systems().contains(&"cargo"));
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
