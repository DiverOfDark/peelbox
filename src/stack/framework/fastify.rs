//! Fastify framework for JavaScript/TypeScript

use super::*;

pub struct FastifyFramework;

impl Framework for FastifyFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Fastify
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["JavaScript".to_string(), "TypeScript".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["npm".to_string(), "yarn".to_string(), "pnpm".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::NpmPackage,
            pattern: "fastify".to_string(),
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
        vec![
            (r"PORT\s*=\s*(\d+)".to_string(), "Fastify port".to_string()),
            (
                r"NODE_ENV\s*=\s*(\w+)".to_string(),
                "Node environment".to_string(),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_fastify_compatibility() {
        let framework = FastifyFramework;

        assert!(framework
            .compatible_languages()
            .iter()
            .any(|s| s == "JavaScript"));
        assert!(framework
            .compatible_languages()
            .iter()
            .any(|s| s == "TypeScript"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "npm"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "yarn"));
    }

    #[test]
    fn test_fastify_dependency_detection() {
        let framework = FastifyFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "fastify".to_string(),
            version: Some("4.20.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_fastify_health_endpoints() {
        let framework = FastifyFramework;
        let endpoints = framework.health_endpoints(&[]);

        assert!(endpoints.iter().any(|s| s == "/health"));
        assert!(endpoints.iter().any(|s| s == "/healthz"));
    }

    #[test]
    fn test_fastify_default_ports() {
        let framework = FastifyFramework;
        assert_eq!(framework.default_ports(), vec![3000]);
    }
}
