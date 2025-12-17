use crate::output::schema::UniversalBuild;
use anyhow::Result;

pub fn validate_required_fields(build: &UniversalBuild) -> Result<()> {
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

pub fn validate_non_empty_commands(build: &UniversalBuild) -> Result<()> {
    if build.build.commands.is_empty() {
        anyhow::bail!("Build commands cannot be empty");
    }
    if build.runtime.command.is_empty() {
        anyhow::bail!("Runtime command cannot be empty");
    }
    Ok(())
}

pub fn validate_valid_image_name(build: &UniversalBuild) -> Result<()> {
    if build.build.base.is_empty() {
        anyhow::bail!("Build base image cannot be empty");
    }
    if build.runtime.base.is_empty() {
        anyhow::bail!("Runtime base image cannot be empty");
    }
    Ok(())
}

pub fn validate_confidence_range(build: &UniversalBuild) -> Result<()> {
    if !(0.0..=1.0).contains(&build.metadata.confidence) {
        anyhow::bail!(
            "Confidence score must be between 0.0 and 1.0, got {}",
            build.metadata.confidence
        );
    }
    Ok(())
}

pub fn validate_non_empty_context(build: &UniversalBuild) -> Result<()> {
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

pub fn validate_non_empty_artifacts(build: &UniversalBuild) -> Result<()> {
    if build.build.artifacts.is_empty() {
        anyhow::bail!("Build artifacts cannot be empty");
    }
    Ok(())
}

pub fn validate_valid_copy_specs(build: &UniversalBuild) -> Result<()> {
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
    fn test_validate_required_fields_valid() {
        let build = create_minimal_valid_build();
        assert!(validate_required_fields(&build).is_ok());
    }

    #[test]
    fn test_validate_required_fields_missing_version() {
        let mut build = create_minimal_valid_build();
        build.version = String::new();
        assert!(validate_required_fields(&build).is_err());
    }

    #[test]
    fn test_validate_non_empty_commands_valid() {
        let build = create_minimal_valid_build();
        assert!(validate_non_empty_commands(&build).is_ok());
    }

    #[test]
    fn test_validate_non_empty_commands_empty_build_commands() {
        let mut build = create_minimal_valid_build();
        build.build.commands = vec![];
        assert!(validate_non_empty_commands(&build).is_err());
    }

    #[test]
    fn test_validate_confidence_range_valid() {
        let build = create_minimal_valid_build();
        assert!(validate_confidence_range(&build).is_ok());
    }

    #[test]
    fn test_validate_confidence_range_invalid() {
        let mut build = create_minimal_valid_build();
        build.metadata.confidence = 1.5;
        assert!(validate_confidence_range(&build).is_err());
    }

    #[test]
    fn test_validate_non_empty_context_valid() {
        let build = create_minimal_valid_build();
        assert!(validate_non_empty_context(&build).is_ok());
    }

    #[test]
    fn test_validate_non_empty_artifacts_valid() {
        let build = create_minimal_valid_build();
        assert!(validate_non_empty_artifacts(&build).is_ok());
    }

    #[test]
    fn test_validate_valid_copy_specs_valid() {
        let build = create_minimal_valid_build();
        assert!(validate_valid_copy_specs(&build).is_ok());
    }
}
