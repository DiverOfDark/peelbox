//! Axum framework for Rust

use super::*;

pub struct AxumFramework;

impl Framework for AxumFramework {
    fn id(&self) -> crate::FrameworkId {
        crate::FrameworkId::Axum
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
            pattern: r"axum".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![3000]
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
    fn test_axum_compatibility() {
        let framework = AxumFramework;
        assert!(framework.compatible_languages().iter().any(|s| s == "Rust"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "cargo"));
    }

    #[test]
    fn test_axum_dependency_detection() {
        let framework = AxumFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "axum".to_string(),
            version: Some("0.7.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_axum_default_ports() {
        let framework = AxumFramework;
        assert_eq!(framework.default_ports(), vec![3000]);
    }
}
