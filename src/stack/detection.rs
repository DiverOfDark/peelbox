use super::{BuildSystemId, FrameworkId, LanguageId};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionStack {
    pub build_system: BuildSystemId,
    pub language: LanguageId,
    pub framework: Option<FrameworkId>,
    pub confidence: f64,
    pub manifest_path: PathBuf,
}

impl DetectionStack {
    pub fn new(
        build_system: BuildSystemId,
        language: LanguageId,
        manifest_path: PathBuf,
    ) -> Self {
        Self {
            build_system,
            language,
            framework: None,
            confidence: 1.0,
            manifest_path,
        }
    }

    pub fn with_framework(mut self, framework: FrameworkId) -> Self {
        self.framework = Some(framework);
        self
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn validate(&self) -> bool {
        self.confidence >= 0.0 && self.confidence <= 1.0
    }

    pub fn to_string_parts(&self) -> (String, String, Option<String>) {
        (
            self.build_system.name().to_string(),
            self.language.name().to_string(),
            self.framework.map(|f| f.name().to_string()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_stack_creation() {
        let stack = DetectionStack::new(
            BuildSystemId::Cargo,
            LanguageId::Rust,
            PathBuf::from("Cargo.toml"),
        );

        assert_eq!(stack.build_system, BuildSystemId::Cargo);
        assert_eq!(stack.language, LanguageId::Rust);
        assert_eq!(stack.framework, None);
        assert_eq!(stack.confidence, 1.0);
    }

    #[test]
    fn test_detection_stack_with_framework() {
        let stack = DetectionStack::new(
            BuildSystemId::Cargo,
            LanguageId::Rust,
            PathBuf::from("Cargo.toml"),
        )
        .with_framework(FrameworkId::ActixWeb);

        assert_eq!(stack.framework, Some(FrameworkId::ActixWeb));
    }

    #[test]
    fn test_detection_stack_validation() {
        let valid = DetectionStack::new(
            BuildSystemId::Cargo,
            LanguageId::Rust,
            PathBuf::from("Cargo.toml"),
        )
        .with_confidence(0.95);
        assert!(valid.validate());

        let invalid = DetectionStack::new(
            BuildSystemId::Cargo,
            LanguageId::Rust,
            PathBuf::from("Cargo.toml"),
        )
        .with_confidence(1.5);
        assert!(!invalid.validate());
    }

    #[test]
    fn test_to_string_parts() {
        let stack = DetectionStack::new(
            BuildSystemId::Maven,
            LanguageId::Java,
            PathBuf::from("pom.xml"),
        )
        .with_framework(FrameworkId::SpringBoot);

        let (bs, lang, fw) = stack.to_string_parts();
        assert_eq!(bs, "Maven");
        assert_eq!(lang, "Java");
        assert_eq!(fw, Some("Spring Boot".to_string()));
    }
}
