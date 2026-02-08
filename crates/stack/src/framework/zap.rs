//! Zap framework for Zig (zigzap/zap, built on facil.io)

use super::*;

pub struct ZapFramework;

impl Framework for ZapFramework {
    fn id(&self) -> crate::FrameworkId {
        crate::FrameworkId::Zap
    }

    fn compatible_languages(&self) -> Vec<String> {
        vec!["Zig".to_string()]
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["zig".to_string()]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![DependencyPattern {
            pattern_type: DependencyPatternType::Regex,
            pattern: r"^zap$".to_string(),
            confidence: 0.95,
        }]
    }

    fn default_ports(&self) -> Vec<u16> {
        vec![3000]
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        vec!["/health".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::Dependency;

    #[test]
    fn test_zap_compatibility() {
        let framework = ZapFramework;
        assert!(framework.compatible_languages().iter().any(|s| s == "Zig"));
        assert!(framework
            .compatible_build_systems()
            .iter()
            .any(|s| s == "zig"));
    }

    #[test]
    fn test_zap_dependency_detection() {
        let framework = ZapFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "zap".to_string(),
            version: None,
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_zap_no_false_positive() {
        let framework = ZapFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "zaplib".to_string(),
            version: None,
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(matches.is_empty());
    }

    #[test]
    fn test_zap_default_ports() {
        let framework = ZapFramework;
        assert_eq!(framework.default_ports(), vec![3000]);
    }
}
