use crate::output::schema::UniversalBuild;
use anyhow::Result;

pub trait ValidationRule: Send + Sync {
    fn name(&self) -> &'static str;
    fn validate(&self, build: &UniversalBuild) -> Result<()>;
}

pub struct RequiredFieldsRule;

impl ValidationRule for RequiredFieldsRule {
    fn name(&self) -> &'static str {
        "RequiredFields"
    }

    fn validate(&self, build: &UniversalBuild) -> Result<()> {
        if build.version.is_empty() {
            anyhow::bail!("Version cannot be empty");
        }
        if build.metadata.language.is_empty() {
            anyhow::bail!("Language cannot be empty");
        }
        if build.metadata.build_system.is_empty() {
            anyhow::bail!("Build system cannot be empty");
        }
        if build.build.base.is_empty() {
            anyhow::bail!("Build base image cannot be empty");
        }
        if build.runtime.base.is_empty() {
            anyhow::bail!("Runtime base image cannot be empty");
        }
        Ok(())
    }
}

pub struct NonEmptyCommandsRule;

impl ValidationRule for NonEmptyCommandsRule {
    fn name(&self) -> &'static str {
        "NonEmptyCommands"
    }

    fn validate(&self, build: &UniversalBuild) -> Result<()> {
        if build.build.commands.is_empty() {
            anyhow::bail!("Build commands cannot be empty");
        }
        if build.runtime.command.is_empty() {
            anyhow::bail!("Runtime command cannot be empty");
        }
        Ok(())
    }
}

pub struct ValidImageNameRule;

impl ValidationRule for ValidImageNameRule {
    fn name(&self) -> &'static str {
        "ValidImageName"
    }

    fn validate(&self, build: &UniversalBuild) -> Result<()> {
        if build.build.base.is_empty() {
            anyhow::bail!("Build base image cannot be empty");
        }
        if build.runtime.base.is_empty() {
            anyhow::bail!("Runtime base image cannot be empty");
        }
        Ok(())
    }
}

pub struct ConfidenceRangeRule;

impl ValidationRule for ConfidenceRangeRule {
    fn name(&self) -> &'static str {
        "ConfidenceRange"
    }

    fn validate(&self, build: &UniversalBuild) -> Result<()> {
        if !(0.0..=1.0).contains(&build.metadata.confidence) {
            anyhow::bail!(
                "Confidence score must be between 0.0 and 1.0, got {}",
                build.metadata.confidence
            );
        }
        Ok(())
    }
}

pub struct NonEmptyContextRule;

impl ValidationRule for NonEmptyContextRule {
    fn name(&self) -> &'static str {
        "NonEmptyContext"
    }

    fn validate(&self, build: &UniversalBuild) -> Result<()> {
        if build.build.context.is_empty() {
            anyhow::bail!("Build context cannot be empty");
        }
        for (i, context_spec) in build.build.context.iter().enumerate() {
            if context_spec.from.is_empty() {
                anyhow::bail!("Build context[{}] 'from' path cannot be empty", i);
            }
            if context_spec.to.is_empty() {
                anyhow::bail!("Build context[{}] 'to' path cannot be empty", i);
            }
        }
        Ok(())
    }
}

pub struct NonEmptyArtifactsRule;

impl ValidationRule for NonEmptyArtifactsRule {
    fn name(&self) -> &'static str {
        "NonEmptyArtifacts"
    }

    fn validate(&self, build: &UniversalBuild) -> Result<()> {
        if build.build.artifacts.is_empty() {
            anyhow::bail!("Build artifacts cannot be empty");
        }
        Ok(())
    }
}

pub struct ValidCopySpecsRule;

impl ValidationRule for ValidCopySpecsRule {
    fn name(&self) -> &'static str {
        "ValidCopySpecs"
    }

    fn validate(&self, build: &UniversalBuild) -> Result<()> {
        if build.runtime.copy.is_empty() {
            anyhow::bail!("Runtime copy specifications cannot be empty");
        }
        for (i, copy_spec) in build.runtime.copy.iter().enumerate() {
            if copy_spec.from.is_empty() {
                anyhow::bail!("Runtime copy[{}] 'from' path cannot be empty", i);
            }
            if copy_spec.to.is_empty() {
                anyhow::bail!("Runtime copy[{}] 'to' path cannot be empty", i);
            }
        }
        Ok(())
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
    fn test_required_fields_rule_valid() {
        let build = create_minimal_valid_build();
        let rule = RequiredFieldsRule;
        assert!(rule.validate(&build).is_ok());
    }

    #[test]
    fn test_required_fields_rule_missing_version() {
        let mut build = create_minimal_valid_build();
        build.version = String::new();
        let rule = RequiredFieldsRule;
        assert!(rule.validate(&build).is_err());
    }

    #[test]
    fn test_non_empty_commands_rule_valid() {
        let build = create_minimal_valid_build();
        let rule = NonEmptyCommandsRule;
        assert!(rule.validate(&build).is_ok());
    }

    #[test]
    fn test_non_empty_commands_rule_empty_build_commands() {
        let mut build = create_minimal_valid_build();
        build.build.commands = vec![];
        let rule = NonEmptyCommandsRule;
        assert!(rule.validate(&build).is_err());
    }

    #[test]
    fn test_confidence_range_rule_valid() {
        let build = create_minimal_valid_build();
        let rule = ConfidenceRangeRule;
        assert!(rule.validate(&build).is_ok());
    }

    #[test]
    fn test_confidence_range_rule_invalid() {
        let mut build = create_minimal_valid_build();
        build.metadata.confidence = 1.5;
        let rule = ConfidenceRangeRule;
        assert!(rule.validate(&build).is_err());
    }

    #[test]
    fn test_non_empty_context_rule_valid() {
        let build = create_minimal_valid_build();
        let rule = NonEmptyContextRule;
        assert!(rule.validate(&build).is_ok());
    }

    #[test]
    fn test_non_empty_artifacts_rule_valid() {
        let build = create_minimal_valid_build();
        let rule = NonEmptyArtifactsRule;
        assert!(rule.validate(&build).is_ok());
    }

    #[test]
    fn test_valid_copy_specs_rule_valid() {
        let build = create_minimal_valid_build();
        let rule = ValidCopySpecsRule;
        assert!(rule.validate(&build).is_ok());
    }
}
