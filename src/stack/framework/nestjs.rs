//! NestJS framework for TypeScript

use super::*;

pub struct NestJsFramework;

impl Framework for NestJsFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::NestJs
    }

    fn compatible_languages(&self) -> &[&str] {
        &["TypeScript", "JavaScript"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["npm", "yarn", "pnpm"]
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

    fn default_ports(&self) -> &[u16] {
        &[3000]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/health/liveness", "/health/readiness"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"PORT\s*=\s*(\d+)", "NestJS port"),
            (r"NODE_ENV\s*=\s*(\w+)", "Node environment"),
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

        assert!(framework.compatible_languages().contains(&"TypeScript"));
        assert!(framework.compatible_languages().contains(&"JavaScript"));
        assert!(framework.compatible_build_systems().contains(&"npm"));
        assert!(framework.compatible_build_systems().contains(&"yarn"));
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

        assert!(endpoints.contains(&"/health"));
        assert!(endpoints.contains(&"/health/liveness"));
    }

    #[test]
    fn test_nestjs_default_ports() {
        let framework = NestJsFramework;
        assert_eq!(framework.default_ports(), &[3000]);
    }
}
