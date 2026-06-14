use peelbox_core::output::schema::UniversalBuild;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tracing::info;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Repository path not found: {0}")]
    PathNotFound(PathBuf),

    #[error("Repository path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Detection failed: {0}")]
    DetectionFailed(String),
}

impl ServiceError {
    pub fn help_message(&self) -> String {
        match self {
            ServiceError::PathNotFound(path) => {
                format!(
                    "Error: Repository path not found\nPath: {}\n\n\
                    Help: The specified path does not exist. Please check:\n\
                    - Is the path correct?\n\
                    - Does the path exist on your system?\n\
                    - Do you have permission to access it?",
                    path.display()
                )
            }
            ServiceError::NotADirectory(path) => {
                format!(
                    "Error: Repository path is not a directory\nPath: {}\n\n\
                    Help: The specified path is a file, not a directory.\n\
                    Please provide the path to the repository root directory.",
                    path.display()
                )
            }
            ServiceError::ConfigError(msg) => {
                format!(
                    "Error: Configuration error\n\n\
                    Help: Configuration validation failed. Try:\n\
                    - Check the repository structure\n\
                    - Check the documentation\n\n\
                    Details: {}",
                    msg
                )
            }
            ServiceError::DetectionFailed(msg) => {
                format!(
                    "Error: Detection failed\n\n\
                    Help: The detection process failed. Try:\n\
                    - Retry the operation\n\
                    - Check the repository is valid\n\
                    - Report this issue if it persists\n\n\
                    Details: {}",
                    msg
                )
            }
        }
    }
}

/// Deterministic, static repository detection.
///
/// Walks the repository, parses manifests and configuration files, resolves
/// Wolfi packages, and reduces each detected service into a [`UniversalBuild`].
/// Detection is fully deterministic: no network LLM calls, no API keys, and
/// byte-identical output across runs for the same input.
#[derive(Debug, Default)]
pub struct DetectionService;

impl DetectionService {
    pub fn new() -> Self {
        Self
    }

    pub fn detect(&self, repo_path: PathBuf) -> Result<Vec<UniversalBuild>, ServiceError> {
        let start = Instant::now();

        self.validate_repo_path(&repo_path)?;

        info!(repo = %repo_path.display(), "Starting repository detection");

        let wolfi_index = peelbox_wolfi::WolfiPackageIndex::fetch().map_err(|e| {
            ServiceError::DetectionFailed(format!("Failed to fetch Wolfi package index: {}", e))
        })?;

        let registry = peelbox_detect::Registry::with_defaults();
        let results = peelbox_detect::detect_with_registry_and_wolfi(
            &repo_path,
            &registry,
            Some(&wolfi_index),
        )
        .map_err(|e| ServiceError::DetectionFailed(e.to_string()))?;

        Self::ensure_unique_service_names(&results)?;

        let validator = crate::validation::Validator::with_wolfi_index(Arc::new(wolfi_index));
        for build in &results {
            if let Err(e) = validator.validate(build) {
                return Err(ServiceError::DetectionFailed(format!(
                    "Package validation failed: {}",
                    e
                )));
            }
        }

        let elapsed = start.elapsed();

        info!(
            projects_found = results.len(),
            duration_ms = elapsed.as_millis(),
            "Detection complete"
        );

        Ok(results)
    }

    fn validate_repo_path(&self, path: &Path) -> Result<(), ServiceError> {
        if !path.exists() {
            return Err(ServiceError::PathNotFound(path.to_path_buf()));
        }

        if !path.is_dir() {
            return Err(ServiceError::NotADirectory(path.to_path_buf()));
        }

        Ok(())
    }

    fn ensure_unique_service_names(builds: &[UniversalBuild]) -> Result<(), ServiceError> {
        use std::collections::HashSet;

        let mut seen_names: HashSet<String> = HashSet::new();
        let mut duplicates: Vec<String> = Vec::new();

        for build in builds {
            if let Some(ref name) = build.metadata.project_name {
                if !seen_names.insert(name.clone()) {
                    duplicates.push(name.clone());
                }
            }
        }

        if !duplicates.is_empty() {
            let duplicate_list = duplicates.join(", ");
            return Err(ServiceError::ConfigError(format!(
                "Duplicate service names detected: {}. Each service must have a unique name in monorepo.",
                duplicate_list
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_service_error_display() {
        let error = ServiceError::ConfigError("test error".to_string());
        assert_eq!(error.to_string(), "Configuration error: test error");

        let error = ServiceError::PathNotFound(PathBuf::from("/test/path"));
        assert_eq!(error.to_string(), "Repository path not found: /test/path");

        let error = ServiceError::NotADirectory(PathBuf::from("/test/file"));
        assert_eq!(
            error.to_string(),
            "Repository path is not a directory: /test/file"
        );
    }

    #[test]
    fn test_validate_repo_path_not_exists() {
        let service = DetectionService::new();
        let result = service.validate_repo_path(&PathBuf::from("/nonexistent/path"));
        assert!(matches!(result, Err(ServiceError::PathNotFound(_))));
    }

    #[test]
    fn test_validate_repo_path_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        std::fs::write(&file_path, "content").unwrap();

        let service = DetectionService::new();
        let result = service.validate_repo_path(&file_path);
        assert!(matches!(result, Err(ServiceError::NotADirectory(_))));
    }

    #[test]
    fn test_validate_repo_path_success() {
        let temp_dir = TempDir::new().unwrap();
        let service = DetectionService::new();
        assert!(service.validate_repo_path(temp_dir.path()).is_ok());
    }
}
