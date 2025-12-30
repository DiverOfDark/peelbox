use crate::output::schema::UniversalBuild;
use crate::validation::rules::{
    validate_non_empty_commands,
    validate_required_fields, validate_valid_copy_specs,
    validate_wolfi_packages,
};
use crate::validation::WolfiPackageIndex;
use anyhow::Result;
use std::sync::Arc;

pub struct Validator {
    wolfi_index: Option<Arc<WolfiPackageIndex>>,
}

impl Validator {
    pub fn new() -> Self {
        Self { wolfi_index: None }
    }

    pub fn with_wolfi_index(wolfi_index: Arc<WolfiPackageIndex>) -> Self {
        Self {
            wolfi_index: Some(wolfi_index),
        }
    }

    pub fn validate(&self, build: &UniversalBuild) -> Result<()> {
        validate_required_fields(build).map_err(|e| anyhow::anyhow!("[RequiredFields] {}", e))?;
        validate_non_empty_commands(build)
            .map_err(|e| anyhow::anyhow!("[NonEmptyCommands] {}", e))?;
        validate_valid_copy_specs(build).map_err(|e| anyhow::anyhow!("[ValidCopySpecs] {}", e))?;

        if let Some(wolfi_index) = &self.wolfi_index {
            validate_wolfi_packages(build, wolfi_index)
                .map_err(|e| anyhow::anyhow!("[WolfiPackages] {}", e))?;
        }

        Ok(())
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::schema::{BuildMetadata, BuildStage, CopySpec, RuntimeStage};
    use std::collections::HashMap;

    fn create_minimal_valid_build() -> UniversalBuild {
        UniversalBuild {
            version: "1.0".to_string(),
            metadata: BuildMetadata {
                project_name: Some("test-app".to_string()),
                language: "rust".to_string(),
                build_system: "cargo".to_string(),
                framework: None,
                reasoning: "Detected Cargo.toml".to_string(),
            },
            build: BuildStage {
                packages: vec!["rust".to_string(), "build-base".to_string()],
                env: HashMap::new(),
                commands: vec!["cargo build --release".to_string()],
                cache: vec![],
            },
            runtime: RuntimeStage {
                packages: vec!["glibc".to_string(), "ca-certificates".to_string()],
                env: HashMap::new(),
                copy: vec![CopySpec {
                    from: "target/release/app".to_string(),
                    to: "/usr/local/bin/app".to_string(),
                }],
                command: vec!["app".to_string()],
                ports: vec![],
                health: None,
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
}
