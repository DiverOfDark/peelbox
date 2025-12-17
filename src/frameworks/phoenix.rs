//! Phoenix framework for Elixir

use super::*;

pub struct PhoenixFramework;

impl Framework for PhoenixFramework {
    fn name(&self) -> &str {
        "Phoenix"
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Elixir"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["mix"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"phoenix".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[4000]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/api/health"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"PHX_HOST\s*=\s*(\S+)", "Phoenix host"),
            (r"PORT\s*=\s*(\d+)", "Phoenix port"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_phoenix_compatibility() {
        let framework = PhoenixFramework;
        assert!(framework.compatible_languages().contains(&"Elixir"));
        assert!(framework.compatible_build_systems().contains(&"mix"));
    }

    #[test]
    fn test_phoenix_dependency_detection() {
        let framework = PhoenixFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "phoenix".to_string(),
            version: Some("1.7.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_phoenix_default_ports() {
        let framework = PhoenixFramework;
        assert_eq!(framework.default_ports(), &[4000]);
    }
}
