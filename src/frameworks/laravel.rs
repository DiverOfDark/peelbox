//! Laravel framework for PHP

use super::*;

pub struct LaravelFramework;

impl Framework for LaravelFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Laravel
    }


    fn compatible_languages(&self) -> &[&str] {
        &["PHP"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["composer"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"laravel/framework".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[8000]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/api/health"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"APP_ENV\s*=\s*(\w+)", "Laravel environment"),
            (r"APP_PORT\s*=\s*(\d+)", "Laravel port"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_laravel_compatibility() {
        let framework = LaravelFramework;
        assert!(framework.compatible_languages().contains(&"PHP"));
        assert!(framework.compatible_build_systems().contains(&"composer"));
    }

    #[test]
    fn test_laravel_dependency_detection() {
        let framework = LaravelFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "laravel/framework".to_string(),
            version: Some("10.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_laravel_default_ports() {
        let framework = LaravelFramework;
        assert_eq!(framework.default_ports(), &[8000]);
    }
}
