use crate::output::schema::UniversalBuild;
use crate::validation::rules::{
    ConfidenceRangeRule, NonEmptyArtifactsRule, NonEmptyCommandsRule, NonEmptyContextRule,
    RequiredFieldsRule, ValidCopySpecsRule, ValidImageNameRule, ValidationRule,
};
use anyhow::Result;

pub struct Validator {
    rules: Vec<Box<dyn ValidationRule>>,
}

impl Validator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rules(rules: Vec<Box<dyn ValidationRule>>) -> Self {
        Self { rules }
    }

    pub fn validate(&self, build: &UniversalBuild) -> Result<()> {
        for rule in &self.rules {
            if let Err(e) = rule.validate(build) {
                anyhow::bail!("[{}] {}", rule.name(), e);
            }
        }
        Ok(())
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self {
            rules: vec![
                Box::new(RequiredFieldsRule),
                Box::new(NonEmptyCommandsRule),
                Box::new(ValidImageNameRule),
                Box::new(ConfidenceRangeRule),
                Box::new(NonEmptyContextRule),
                Box::new(NonEmptyArtifactsRule),
                Box::new(ValidCopySpecsRule),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::schema::{BuildMetadata, BuildStage, ContextSpec, CopySpec, RuntimeStage};
    use std::collections::HashMap;

    fn create_minimal_valid_build() -> UniversalBuild {
        UniversalBuild {
            version: "1.0".to_string(),
            metadata: BuildMetadata {
                project_name: Some("test-app".to_string()),
                language: "rust".to_string(),
                build_system: "cargo".to_string(),
                confidence: 0.95,
                reasoning: "Detected Cargo.toml".to_string(),
            },
            build: BuildStage {
                base: "rust:1.75".to_string(),
                packages: vec![],
                env: HashMap::new(),
                commands: vec!["cargo build --release".to_string()],
                context: vec![ContextSpec {
                    from: ".".to_string(),
                    to: "/app".to_string(),
                }],
                cache: vec![],
                artifacts: vec!["target/release/app".to_string()],
            },
            runtime: RuntimeStage {
                base: "debian:bookworm-slim".to_string(),
                packages: vec![],
                env: HashMap::new(),
                copy: vec![CopySpec {
                    from: "target/release/app".to_string(),
                    to: "/usr/local/bin/app".to_string(),
                }],
                command: vec!["app".to_string()],
                ports: vec![],
            },
        }
    }

    #[test]
    fn test_validator_valid_build() {
        let build = create_minimal_valid_build();
        let validator = Validator::new();
        assert!(validator.validate(&build).is_ok());
    }

    #[test]
    fn test_validator_invalid_build_empty_version() {
        let mut build = create_minimal_valid_build();
        build.version = String::new();
        let validator = Validator::new();
        let result = validator.validate(&build);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("RequiredFields"));
    }

    #[test]
    fn test_validator_invalid_build_empty_commands() {
        let mut build = create_minimal_valid_build();
        build.build.commands = vec![];
        let validator = Validator::new();
        let result = validator.validate(&build);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("NonEmptyCommands"));
    }

    #[test]
    fn test_validator_invalid_confidence() {
        let mut build = create_minimal_valid_build();
        build.metadata.confidence = 1.5;
        let validator = Validator::new();
        let result = validator.validate(&build);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ConfidenceRange"));
    }
}
