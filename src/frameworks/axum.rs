//! Axum framework for Rust

use super::*;

pub struct AxumFramework;

impl Framework for AxumFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Axum
    }


    fn compatible_languages(&self) -> &[&str] {
        &["Rust"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["cargo"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"axum".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[3000]
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
    fn test_axum_compatibility() {
        let framework = AxumFramework;
        assert!(framework.compatible_languages().contains(&"Rust"));
        assert!(framework.compatible_build_systems().contains(&"cargo"));
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
        assert_eq!(framework.default_ports(), &[3000]);
    }
}
