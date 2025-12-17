//! Next.js framework for JavaScript/TypeScript

use super::*;

pub struct NextJsFramework;

impl Framework for NextJsFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::NextJs
    }


    fn compatible_languages(&self) -> &[&str] {
        &["JavaScript", "TypeScript"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["npm", "yarn", "pnpm", "bun"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::NpmPackage,
                pattern: "next".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[3000]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/api/health", "/health"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"PORT\s*=\s*(\d+)", "Next.js port"),
            (r"NODE_ENV\s*=\s*(\w+)", "Node environment"),
        ]
    }

    fn customize_build_template(&self, mut template: BuildTemplate) -> BuildTemplate {
        if !template.artifacts.iter().any(|a| a.contains(".next")) {
            template.artifacts.push(".next/".to_string());
            template.artifacts.push("public/".to_string());
        }
        template
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_nextjs_compatibility() {
        let framework = NextJsFramework;

        assert!(framework.compatible_languages().contains(&"JavaScript"));
        assert!(framework.compatible_languages().contains(&"TypeScript"));
        assert!(framework.compatible_build_systems().contains(&"npm"));
        assert!(framework.compatible_build_systems().contains(&"yarn"));
        assert!(framework.compatible_build_systems().contains(&"pnpm"));
    }

    #[test]
    fn test_nextjs_dependency_detection() {
        let framework = NextJsFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "next".to_string(),
            version: Some("14.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_nextjs_health_endpoints() {
        let framework = NextJsFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/api/health"));
        assert!(endpoints.contains(&"/health"));
    }

    #[test]
    fn test_nextjs_default_ports() {
        let framework = NextJsFramework;
        assert_eq!(framework.default_ports(), &[3000]);
    }
}
