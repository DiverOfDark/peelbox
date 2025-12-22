//! NestJS framework for TypeScript

use super::*;

pub struct NestJsFramework;

impl Framework for NestJsFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::NestJs
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["TypeScript".to_string(), "JavaScript".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["npm".to_string(), "yarn".to_string(), "pnpm".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::NpmPackage,
                pattern: "@nestjs/core".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::NpmPackage,
                pattern: "@nestjs/common".to_string(),
                confidence: 0.9,
            },
        ]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![3000]
    }

    fn health_endpoints(&self) -> Vec<String> {
        vec!["/health".to_string(), "/health/liveness".to_string(), "/health/readiness".to_string()]
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (r"PORT\s*=\s*(\d+)".to_string(), "NestJS port".to_string()),
            (r"NODE_ENV\s*=\s*(\w+)".to_string(), "Node environment".to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_nestjs_compatibility() {
        let framework = NestJsFramework;

        assert!(framework.compatible_languages().iter().any(|s| s == "TypeScript"));
        assert!(framework.compatible_languages().iter().any(|s| s == "JavaScript"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "npm"));
        assert!(framework.compatible_build_systems().iter().any(|s| s == "yarn"));
    }

    #[test]
    fn test_nestjs_dependency_detection() {
        let framework = NestJsFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "@nestjs/core".to_string(),
            version: Some("10.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_nestjs_health_endpoints() {
        let framework = NestJsFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/health/liveness"));
    }

    #[test]
    fn test_nestjs_default_ports() {
        let framework = NestJsFramework;
        assert_eq!(framework.default_ports(), vec![3000]);
    }
}
