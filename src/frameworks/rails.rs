//! Ruby on Rails framework

use super::*;

pub struct RailsFramework;

impl Framework for RailsFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Rails
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Ruby"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["bundler"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"^rails$".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> &[u16] {
        &[3000]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz", "/up"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"RAILS_ENV\s*=\s*(\w+)", "Rails environment"),
            (r"PORT\s*=\s*(\d+)", "Rails port"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_rails_compatibility() {
        let framework = RailsFramework;

        assert!(framework.compatible_languages().contains(&"Ruby"));
        assert!(framework.compatible_build_systems().contains(&"bundler"));
    }

    #[test]
    fn test_rails_dependency_detection() {
        let framework = RailsFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "rails".to_string(),
            version: Some("7.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_rails_health_endpoints() {
        let framework = RailsFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/health"));
        assert!(endpoints.contains(&"/up"));
    }

    #[test]
    fn test_rails_default_ports() {
        let framework = RailsFramework;
        assert_eq!(framework.default_ports(), &[3000]);
    }
}
