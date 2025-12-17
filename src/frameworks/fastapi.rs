//! FastAPI framework for Python

use super::*;

pub struct FastApiFramework;

impl Framework for FastApiFramework {
    fn name(&self) -> &str {
        "FastAPI"
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Python"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["pip", "poetry", "pipenv"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::PypiPackage,
                pattern: "fastapi".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[8000]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz", "/docs"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"PORT\s*=\s*(\d+)", "FastAPI port"),
            (r"ENVIRONMENT\s*=\s*(\w+)", "FastAPI environment"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_fastapi_compatibility() {
        let framework = FastApiFramework;

        assert!(framework.compatible_languages().contains(&"Python"));
        assert!(framework.compatible_build_systems().contains(&"pip"));
        assert!(framework.compatible_build_systems().contains(&"poetry"));
    }

    #[test]
    fn test_fastapi_dependency_detection() {
        let framework = FastApiFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "fastapi".to_string(),
            version: Some("0.104.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_fastapi_health_endpoints() {
        let framework = FastApiFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/health"));
        assert!(endpoints.contains(&"/docs"));
    }

    #[test]
    fn test_fastapi_default_ports() {
        let framework = FastApiFramework;
        assert_eq!(framework.default_ports(), &[8000]);
    }
}
