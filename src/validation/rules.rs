use crate::output::schema::UniversalBuild;
use crate::validation::WolfiPackageIndex;
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

pub fn validate_wolfi_packages(
    build: &UniversalBuild,
    wolfi_index: &WolfiPackageIndex,
) -> Result<()> {
    let mut errors = Vec::new();

    for package in &build.build.packages {
        if let Some(error) = validate_package(package, wolfi_index) {
            errors.push(format!("Build package: {}", error));
        }
    }

    for package in &build.runtime.packages {
        if let Some(error) = validate_package(package, wolfi_index) {
            errors.push(format!("Runtime package: {}", error));
        }
    }

    if !errors.is_empty() {
        anyhow::bail!("Wolfi package validation failed:\n  {}", errors.join("\n  "));
    }

    Ok(())
}

fn validate_package(package: &str, wolfi_index: &WolfiPackageIndex) -> Option<String> {
    if wolfi_index.has_package(package) {
        return None;
    }

    if is_version_less_package(package) {
        let base_name = extract_base_name(package);
        let versions = wolfi_index.get_versions(&base_name);

        if !versions.is_empty() {
            let suggestions: Vec<String> = versions
                .iter()
                .take(5)
                .map(|v| format!("{}-{}", base_name, v))
                .collect();

            return Some(format!(
                "Package '{}' not found. Did you mean: {}?",
                package,
                suggestions.join(", ")
            ));
        }
    }

    let suggestions = find_similar_packages(package, wolfi_index, 3);
    if !suggestions.is_empty() {
        return Some(format!(
            "Package '{}' not found. Did you mean: {}?",
            package,
            suggestions.join(", ")
        ));
    }

    Some(format!("Package '{}' not found in Wolfi repository", package))
}

fn is_version_less_package(package: &str) -> bool {
    let common_version_less = [
        "nodejs", "python", "openjdk", "ruby", "php", "go", "rust", "dotnet", "elixir",
    ];

    common_version_less.contains(&package)
}

fn extract_base_name(package: &str) -> String {
    package.to_string()
}

fn find_similar_packages(
    package: &str,
    wolfi_index: &WolfiPackageIndex,
    max_suggestions: usize,
) -> Vec<String> {
    use strsim::levenshtein;

    let all_packages = wolfi_index.all_packages();

    let mut scored: Vec<(String, usize)> = all_packages
        .iter()
        .map(|p| {
            let distance = levenshtein(package, p);
            (p.clone(), distance)
        })
        .collect();

    scored.sort_by_key(|(_, dist)| *dist);

    scored
        .iter()
        .filter(|(_, dist)| *dist <= 3)
        .take(max_suggestions)
        .map(|(pkg, _)| pkg.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::schema::{BuildMetadata, BuildStage, ContextSpec, CopySpec, RuntimeStage};
    use std::collections::HashMap;

    fn create_minimal_valid_build() -> UniversalBuild {
        // Get actual Rust version from test APKINDEX for validation tests
        let wolfi_index = crate::validation::WolfiPackageIndex::for_tests();
        let rust_package = wolfi_index
            .get_latest_version("rust")
            .unwrap_or_else(|| "rust-1.92".to_string());

        UniversalBuild {
            version: "1.0".to_string(),
            metadata: BuildMetadata {
                project_name: Some("test-app".to_string()),
                language: "rust".to_string(),
                build_system: "cargo".to_string(),
                framework: None,
                confidence: 0.95,
                reasoning: "Detected Cargo.toml".to_string(),
            },
            build: BuildStage {
                packages: vec![rust_package, "build-base".to_string()],
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

    #[test]
    fn test_validate_wolfi_packages_valid_versioned() {
        let build = create_minimal_valid_build();
        let wolfi_index = crate::validation::WolfiPackageIndex::for_tests();

        assert!(validate_wolfi_packages(&build, &wolfi_index).is_ok());
    }

    #[test]
    fn test_validate_wolfi_packages_invalid_version_less() {
        let mut build = create_minimal_valid_build();
        build.build.packages = vec!["nodejs".to_string()];

        let wolfi_index = crate::validation::WolfiPackageIndex::for_tests();
        let result = validate_wolfi_packages(&build, &wolfi_index);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("nodejs"));
        assert!(err_msg.contains("Did you mean"));
    }

    #[test]
    fn test_validate_wolfi_packages_typo_fuzzy_match() {
        let mut build = create_minimal_valid_build();
        build.runtime.packages = vec!["gliibc".to_string()];

        let wolfi_index = crate::validation::WolfiPackageIndex::for_tests();
        let result = validate_wolfi_packages(&build, &wolfi_index);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("gliibc"));
        assert!(err_msg.contains("Did you mean"));
        assert!(err_msg.contains("glibc"));
    }

    #[test]
    fn test_validate_wolfi_packages_completely_invalid() {
        let mut build = create_minimal_valid_build();
        build.build.packages = vec!["nonexistent-package-12345".to_string()];

        let wolfi_index = crate::validation::WolfiPackageIndex::for_tests();
        let result = validate_wolfi_packages(&build, &wolfi_index);

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("nonexistent-package-12345"));
        assert!(err_msg.contains("not found"));
    }

    #[test]
    fn test_validate_wolfi_packages_valid_generic() {
        let mut build = create_minimal_valid_build();
        build.runtime.packages = vec!["glibc".to_string(), "ca-certificates".to_string()];

        let wolfi_index = crate::validation::WolfiPackageIndex::for_tests();
        assert!(validate_wolfi_packages(&build, &wolfi_index).is_ok());
    }

    #[test]
    fn test_is_version_less_package() {
        assert!(is_version_less_package("nodejs"));
        assert!(is_version_less_package("python"));
        assert!(is_version_less_package("openjdk"));
        assert!(!is_version_less_package("nodejs-22"));
        assert!(!is_version_less_package("glibc"));
    }
}
