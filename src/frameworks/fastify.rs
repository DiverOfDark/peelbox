//! Fastify framework for JavaScript/TypeScript

use super::*;

pub struct FastifyFramework;

impl Framework for FastifyFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Fastify
    }


    fn compatible_languages(&self) -> &[&str] {
        &["JavaScript", "TypeScript"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["npm", "yarn", "pnpm"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::NpmPackage,
                pattern: "fastify".to_string(),
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
            (r"PORT\s*=\s*(\d+)", "Fastify port"),
            (r"NODE_ENV\s*=\s*(\w+)", "Node environment"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_fastify_compatibility() {
        let framework = FastifyFramework;

        assert!(framework.compatible_languages().contains(&"JavaScript"));
        assert!(framework.compatible_languages().contains(&"TypeScript"));
        assert!(framework.compatible_build_systems().contains(&"npm"));
        assert!(framework.compatible_build_systems().contains(&"yarn"));
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
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/health"));
        assert!(endpoints.contains(&"/healthz"));
    }

    #[test]
    fn test_fastify_default_ports() {
        let framework = FastifyFramework;
        assert_eq!(framework.default_ports(), &[3000]);
    }
}
