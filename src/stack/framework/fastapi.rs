//! FastAPI framework for Python

use super::*;

pub struct FastApiFramework;

impl Framework for FastApiFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::FastApi
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Python".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["pip".to_string(), "poetry".to_string(), "pipenv".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::PypiPackage,
            pattern: "fastapi".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> &[u16] {
        &[8000]
    }

    fn health_endpoints(&self) -> Vec<String> {
        vec!["/health".to_string(), "/healthz".to_string(), "/docs".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"PORT\s*=\s*(\d+)".to_string(), "FastAPI port".to_string()),
            (r"ENVIRONMENT\s*=\s*(\w+)".to_string(), "FastAPI environment".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_fastapi_compatibility() {
        let framework = FastApiFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "Python"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "pip"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "poetry"));
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

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/docs"));
    }

    #[test]
    fn test_fastapi_default_ports() {
        let framework = FastApiFramework;
        assert_eq!(framework.default_ports(), &[8000]);
    }
}
