//! Gin framework for Go

use super::*;

pub struct GinFramework;

impl Framework for GinFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::Gin
    }


    fn compatible_languages(&self) -> &[&str] {
        &["Go"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["go"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"github\.com/gin-gonic/gin".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[8080]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz", "/ping"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"GIN_MODE\s*=\s*(\w+)", "Gin mode"),
            (r"PORT\s*=\s*(\d+)", "Server port"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_gin_compatibility() {
        let framework = GinFramework;
        assert!(framework.compatible_languages().contains(&"Go"));
        assert!(framework.compatible_build_systems().contains(&"go"));
    }

    #[test]
    fn test_gin_dependency_detection() {
        let framework = GinFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "github.com/gin-gonic/gin".to_string(),
            version: Some("v1.9.1".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_gin_default_ports() {
        let framework = GinFramework;
        assert_eq!(framework.default_ports(), &[8080]);
    }
}
