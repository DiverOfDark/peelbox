//! Sinatra framework for Ruby

use super::*;

pub struct SinatraFramework;

impl Framework for SinatraFramework {
    fn name(&self) -> &str {
        "Sinatra"
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Ruby"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["bundler"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"^sinatra$".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[4567]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"PORT\s*=\s*(\d+)", "Server port"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_sinatra_compatibility() {
        let framework = SinatraFramework;
        assert!(framework.compatible_languages().contains(&"Ruby"));
        assert!(framework.compatible_build_systems().contains(&"bundler"));
    }

    #[test]
    fn test_sinatra_dependency_detection() {
        let framework = SinatraFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "sinatra".to_string(),
            version: Some("3.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_sinatra_default_ports() {
        let framework = SinatraFramework;
        assert_eq!(framework.default_ports(), &[4567]);
    }
}
