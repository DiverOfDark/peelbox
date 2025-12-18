//! ASP.NET Core framework for .NET

use super::*;

pub struct AspNetFramework;

impl Framework for AspNetFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::AspNetCore
    }

    fn compatible_languages(&self) -> &[&str] {
        &["C#", "F#"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["dotnet"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"Microsoft\.AspNetCore\..*".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"Microsoft\.Extensions\..*".to_string(),
                confidence: 0.85,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[5000, 5001]
    }

    fn health_endpoints(&self) -> &[&str] {
        &["/health", "/healthz", "/ready"]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"ASPNETCORE_ENVIRONMENT\s*=\s*(\w+)", "ASP.NET environment"),
            (r"ASPNETCORE_URLS\s*=\s*([^\s]+)", "ASP.NET URLs"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_aspnet_compatibility() {
        let framework = AspNetFramework;

        assert!(framework.compatible_languages().contains(&"C#"));
        assert!(framework.compatible_languages().contains(&"F#"));
        assert!(framework.compatible_build_systems().contains(&"dotnet"));
    }

    #[test]
    fn test_aspnet_dependency_detection() {
        let framework = AspNetFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "Microsoft.AspNetCore.Mvc".to_string(),
            version: Some("7.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_aspnet_health_endpoints() {
        let framework = AspNetFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/health"));
        assert!(endpoints.contains(&"/ready"));
    }

    #[test]
    fn test_aspnet_default_ports() {
        let framework = AspNetFramework;
        assert_eq!(framework.default_ports(), &[5000, 5001]);
    }
}
