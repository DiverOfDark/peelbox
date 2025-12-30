//! Sinatra framework for Ruby

use super::*;

pub struct SinatraFramework;

impl Framework for SinatraFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Sinatra
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Ruby".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["bundler".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"^sinatra$".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![4567]
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
    use crate::stack::language::Dependency;

    #[test]
    fn test_sinatra_compatibility() {
        let framework = SinatraFramework;
        assert!(framework.compatible_languages().iter().any(|s| s == "Ruby"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "bundler"));
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
        assert_eq!(framework.default_ports(), vec![4567]);
    }
}
