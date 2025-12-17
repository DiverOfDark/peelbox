//! Framework registry for detection and lookup

use super::*;
use crate::languages::DependencyInfo;

/// Registry of all available frameworks
pub struct FrameworkRegistry {
    frameworks: Vec<Box<dyn Framework>>,
}

impl FrameworkRegistry {
    /// Create a new registry with all frameworks
    pub fn new() -> Self {
        let frameworks: Vec<Box<dyn Framework>> = vec![
            Box::new(SpringBootFramework),
            Box::new(ExpressFramework),
            Box::new(DjangoFramework),
            Box::new(RailsFramework),
            Box::new(AspNetFramework),
        ];

        Self { frameworks }
    }

    /// Detect framework from dependencies
    ///
    /// Returns the framework with the highest confidence match, or None if no match found.
    pub fn detect_from_dependencies(&self, deps: &DependencyInfo) -> Option<(&dyn Framework, f32)> {
        let mut best_match: Option<(&dyn Framework, f32)> = None;

        for framework in &self.frameworks {
            for pattern in framework.dependency_patterns() {
                // Check if any dependency matches this pattern
                for dep in &deps.external_deps {
                    if pattern.matches(dep) {
                        let confidence = pattern.confidence;

                        // Update best match if this is better
                        if let Some((_, best_conf)) = best_match {
                            if confidence > best_conf {
                                best_match = Some((framework.as_ref(), confidence));
                            }
                        } else {
                            best_match = Some((framework.as_ref(), confidence));
                        }
                    }
                }
            }
        }

        best_match
    }

    /// Get framework by name
    pub fn get_by_name(&self, name: &str) -> Option<&dyn Framework> {
        self.frameworks
            .iter()
            .find(|f| f.name().eq_ignore_ascii_case(name))
            .map(|f| f.as_ref())
    }

    /// Get all frameworks
    pub fn all_frameworks(&self) -> &[Box<dyn Framework>] {
        &self.frameworks
    }

    /// Validate that a language-framework-build system combination is compatible
    pub fn validate_compatibility(
        &self,
        language: &str,
        framework_name: &str,
        build_system: &str,
    ) -> bool {
        if let Some(framework) = self.get_by_name(framework_name) {
            let lang_compatible = framework
                .compatible_languages()
                .iter()
                .any(|l| l.eq_ignore_ascii_case(language));

            let build_compatible = framework
                .compatible_build_systems()
                .iter()
                .any(|b| b.eq_ignore_ascii_case(build_system));

            lang_compatible && build_compatible
        } else {
            false
        }
    }
}

impl Default for FrameworkRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::Dependency;

    #[test]
    fn test_framework_detection_spring_boot() {
        let registry = FrameworkRegistry::new();

        let mut deps = DependencyInfo::empty();
        deps.external_deps.push(Dependency {
            name: "org.springframework.boot:spring-boot-starter-web".to_string(),
            version: Some("3.0.0".to_string()),
            is_internal: false,
        });

        let result = registry.detect_from_dependencies(&deps);
        assert!(result.is_some());

        let (framework, confidence) = result.unwrap();
        assert_eq!(framework.name(), "Spring Boot");
        assert!(confidence >= 0.9);
    }

    #[test]
    fn test_framework_detection_express() {
        let registry = FrameworkRegistry::new();

        let mut deps = DependencyInfo::empty();
        deps.external_deps.push(Dependency {
            name: "express".to_string(),
            version: Some("4.18.0".to_string()),
            is_internal: false,
        });

        let result = registry.detect_from_dependencies(&deps);
        assert!(result.is_some());

        let (framework, confidence) = result.unwrap();
        assert_eq!(framework.name(), "Express");
        assert!(confidence >= 0.9);
    }

    #[test]
    fn test_get_by_name() {
        let registry = FrameworkRegistry::new();

        let spring = registry.get_by_name("Spring Boot");
        assert!(spring.is_some());
        assert_eq!(spring.unwrap().name(), "Spring Boot");

        let express = registry.get_by_name("express");
        assert!(express.is_some());
        assert_eq!(express.unwrap().name(), "Express");
    }

    #[test]
    fn test_validate_compatibility() {
        let registry = FrameworkRegistry::new();

        // Valid: Spring Boot + Java + Maven
        assert!(registry.validate_compatibility("Java", "Spring Boot", "maven"));

        // Valid: Spring Boot + Kotlin + Gradle
        assert!(registry.validate_compatibility("Kotlin", "Spring Boot", "gradle"));

        // Invalid: Spring Boot + Python
        assert!(!registry.validate_compatibility("Python", "Spring Boot", "pip"));

        // Invalid: Express + Java
        assert!(!registry.validate_compatibility("Java", "Express", "maven"));
    }
}
