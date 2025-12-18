//! Spring Boot framework for Java/Kotlin

use super::*;

pub struct SpringBootFramework;

impl Framework for SpringBootFramework {
    fn id(&self) -> crate::stack::FrameworkId {
        crate::stack::FrameworkId::SpringBoot
    }

    fn compatible_languages(&self) -> &[&str] {
        &["Java", "Kotlin"]
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["maven", "gradle"]
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "org.springframework.boot:spring-boot-starter-web".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "org.springframework.boot:spring-boot-starter".to_string(),
                confidence: 0.9,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"org\.springframework\.boot:spring-boot-starter-.*".to_string(),
                confidence: 0.85,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] {
        &[8080]
    }

    fn health_endpoints(&self) -> &[&str] {
        &[
            "/actuator/health",
            "/actuator/health/liveness",
            "/actuator/health/readiness",
        ]
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"SERVER_PORT\s*=\s*(\d+)", "Spring Boot server.port"),
            (r"SPRING_PROFILES_ACTIVE\s*=\s*(\w+)", "Spring profiles"),
        ]
    }

    fn customize_build_template(&self, mut template: BuildTemplate) -> BuildTemplate {
        // Spring Boot creates fat JARs - adjust artifact path
        if template.artifacts.is_empty() || !template.artifacts.iter().any(|a| a.contains(".jar")) {
            template.artifacts.push("target/*.jar".to_string());
        }
        template
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::language::Dependency;

    #[test]
    fn test_spring_boot_compatibility() {
        let framework = SpringBootFramework;

        assert!(framework.compatible_languages().contains(&"Java"));
        assert!(framework.compatible_languages().contains(&"Kotlin"));
        assert!(framework.compatible_build_systems().contains(&"maven"));
        assert!(framework.compatible_build_systems().contains(&"gradle"));
    }

    #[test]
    fn test_spring_boot_dependency_detection() {
        let framework = SpringBootFramework;
        let patterns = framework.dependency_patterns();

        let dep = Dependency {
            name: "org.springframework.boot:spring-boot-starter-web".to_string(),
            version: Some("3.0.0".to_string()),
            is_internal: false,
        };

        let matches: Vec<_> = patterns.iter().filter(|p| p.matches(&dep)).collect();
        assert!(!matches.is_empty());
        assert!(matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_spring_boot_health_endpoints() {
        let framework = SpringBootFramework;
        let endpoints = framework.health_endpoints();

        assert!(endpoints.contains(&"/actuator/health"));
        assert!(endpoints.contains(&"/actuator/health/liveness"));
    }

    #[test]
    fn test_spring_boot_default_ports() {
        let framework = SpringBootFramework;
        assert_eq!(framework.default_ports(), &[8080]);
    }
}
