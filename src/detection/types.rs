//! Detection data models and types
//!
//! This module contains the core data structures used throughout the detection
//! process, including repository context, detection results, and metadata.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::SystemTime;

/// Git repository metadata
///
/// Contains information about the Git state of a repository, useful for
/// context and debugging purposes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitInfo {
    /// Current branch name (e.g., "main", "develop")
    pub branch: String,

    /// Current commit hash (short or full SHA)
    pub commit_hash: String,

    /// Timestamp of the last commit
    pub last_modified: SystemTime,
}

impl GitInfo {
    /// Creates a new GitInfo instance
    pub fn new(branch: String, commit_hash: String, last_modified: SystemTime) -> Self {
        Self {
            branch,
            commit_hash,
            last_modified,
        }
    }
}

impl fmt::Display for GitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "branch: {}, commit: {}", self.branch, self.commit_hash)
    }
}

/// Repository context provided to the LLM for detection
///
/// This structure aggregates all the information needed for the LLM to make
/// an informed decision about the build system and commands.
///
/// # Example
///
/// ```ignore
/// use std::collections::HashMap;
/// use std::path::PathBuf;
/// use aipack::detection::types::RepositoryContext;
///
/// let mut key_files = HashMap::new();
/// key_files.insert(
///     "Cargo.toml".to_string(),
///     "[package]\nname = \"myproject\"".to_string(),
/// );
///
/// let context = RepositoryContext {
///     file_tree: "myproject/\n├── Cargo.toml\n└── src/".to_string(),
///     key_files,
///     readme_content: Some("# My Project".to_string()),
///     detected_files: vec!["Cargo.toml".to_string()],
///     repo_path: PathBuf::from("/path/to/myproject"),
///     git_info: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryContext {
    /// Visual tree representation of the repository structure
    ///
    /// This should be a human-readable tree (e.g., output of `tree` command)
    /// that helps the LLM understand the project layout.
    pub file_tree: String,

    /// Map of important configuration file paths to their contents
    ///
    /// Keys are relative file paths (e.g., "Cargo.toml", "package.json")
    /// Values are the complete file contents
    pub key_files: HashMap<String, String>,

    /// Optional README.md content
    ///
    /// The README often contains build/setup instructions that can help
    /// the LLM make better decisions.
    pub readme_content: Option<String>,

    /// List of detected configuration files
    ///
    /// File names that were identified as potentially relevant for build
    /// system detection (e.g., ["Cargo.toml", "Makefile"])
    pub detected_files: Vec<String>,

    /// Absolute path to the repository root
    pub repo_path: PathBuf,

    /// Optional Git metadata
    ///
    /// Useful for context but not required for detection
    pub git_info: Option<GitInfo>,
}

impl RepositoryContext {
    /// Creates a minimal RepositoryContext for testing or simple cases
    pub fn minimal(repo_path: PathBuf, file_tree: String) -> Self {
        Self {
            file_tree,
            key_files: HashMap::new(),
            readme_content: None,
            detected_files: Vec::new(),
            repo_path,
            git_info: None,
        }
    }

    /// Adds a key file to the context
    pub fn with_key_file(mut self, path: String, content: String) -> Self {
        self.key_files.insert(path.clone(), content);
        if !self.detected_files.contains(&path) {
            self.detected_files.push(path);
        }
        self
    }

    /// Sets the README content
    pub fn with_readme(mut self, content: String) -> Self {
        self.readme_content = Some(content);
        self
    }

    /// Sets Git information
    pub fn with_git_info(mut self, git_info: GitInfo) -> Self {
        self.git_info = Some(git_info);
        self
    }

    /// Returns the number of key files in this context
    pub fn key_file_count(&self) -> usize {
        self.key_files.len()
    }

    /// Checks if a specific file exists in the key files
    pub fn has_file(&self, filename: &str) -> bool {
        self.key_files.contains_key(filename)
    }
}

impl fmt::Display for RepositoryContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Repository: {}", self.repo_path.display())?;
        writeln!(f, "Key files: {}", self.key_files.len())?;
        if let Some(ref git_info) = self.git_info {
            writeln!(f, "Git: {}", git_info)?;
        }
        Ok(())
    }
}

/// Result of build system detection
///
/// This structure contains all the information extracted by the LLM about
/// the repository's build system, including commands, confidence, and reasoning.
///
/// # Example
///
/// ```ignore
/// use aipack::detection::types::DetectionResult;
///
/// let result = DetectionResult {
///     build_system: "cargo".to_string(),
///     language: "Rust".to_string(),
///     build_command: "cargo build --release".to_string(),
///     test_command: "cargo test".to_string(),
///     deploy_command: "cargo publish".to_string(),
///     dev_command: Some("cargo watch -x run".to_string()),
///     confidence: 0.95,
///     reasoning: "Detected Cargo.toml with standard Rust project structure".to_string(),
///     warnings: vec![],
///     detected_files: vec!["Cargo.toml".to_string()],
///     processing_time_ms: 1250,
/// };
///
/// println!("{}", result);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectionResult {
    /// Identified build system name
    ///
    /// Examples: "cargo", "npm", "maven", "make", "gradle"
    pub build_system: String,

    /// Primary programming language
    ///
    /// Examples: "Rust", "JavaScript", "Java", "Python", "Go"
    pub language: String,

    /// Command to build the project
    ///
    /// This should be the complete command as it would be typed in the shell
    /// Example: "cargo build --release"
    pub build_command: String,

    /// Command to run tests
    ///
    /// Example: "cargo test"
    pub test_command: String,

    /// Command to deploy or publish the project
    ///
    /// Example: "cargo publish" or "npm publish"
    pub deploy_command: String,

    /// Optional development/watch command
    ///
    /// Command to run during development that auto-reloads on changes
    /// Example: Some("cargo watch -x run")
    pub dev_command: Option<String>,

    /// Confidence score (0.0 - 1.0)
    ///
    /// How confident the LLM is in this detection. Values:
    /// - 0.9-1.0: Very confident (clear indicators)
    /// - 0.7-0.9: Confident (standard project structure)
    /// - 0.5-0.7: Moderate (some ambiguity)
    /// - 0.0-0.5: Low confidence (unusual setup or multiple possibilities)
    pub confidence: f32,

    /// Explanation of why these commands were chosen
    ///
    /// Should be a concise explanation that helps users understand the
    /// detection logic and validate the results.
    pub reasoning: String,

    /// Any warnings or potential issues detected
    ///
    /// Examples: missing dependencies, unusual project structure,
    /// deprecated configurations
    pub warnings: Vec<String>,

    /// List of files that led to this detection
    ///
    /// Relative paths to the key files used in detection
    /// Example: ["Cargo.toml", "Cargo.lock"]
    pub detected_files: Vec<String>,

    /// Time taken to process this detection (in milliseconds)
    pub processing_time_ms: u64,
}

impl DetectionResult {
    /// Creates a new DetectionResult with default values for optional fields
    pub fn new(
        build_system: String,
        language: String,
        build_command: String,
        test_command: String,
        deploy_command: String,
    ) -> Self {
        Self {
            build_system,
            language,
            build_command,
            test_command,
            deploy_command,
            dev_command: None,
            confidence: 0.8,
            reasoning: String::new(),
            warnings: Vec::new(),
            detected_files: Vec::new(),
            processing_time_ms: 0,
        }
    }

    /// Checks if the confidence is high (>= 0.8)
    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }

    /// Checks if the confidence is low (< 0.6)
    pub fn is_low_confidence(&self) -> bool {
        self.confidence < 0.6
    }

    /// Returns whether there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Adds a warning to the result
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Sets the confidence score (clamped to 0.0-1.0 range)
    pub fn set_confidence(&mut self, confidence: f32) {
        self.confidence = confidence.clamp(0.0, 1.0);
    }

    /// Returns a confidence level as a string
    pub fn confidence_level(&self) -> &'static str {
        match self.confidence {
            c if c >= 0.9 => "Very High",
            c if c >= 0.8 => "High",
            c if c >= 0.7 => "Moderate",
            c if c >= 0.6 => "Low",
            _ => "Very Low",
        }
    }
}

impl fmt::Display for DetectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Build System Detection Result")?;
        writeln!(f, "==============================")?;
        writeln!(f, "Build System: {}", self.build_system)?;
        writeln!(f, "Language: {}", self.language)?;
        writeln!(
            f,
            "Confidence: {:.1}% ({})",
            self.confidence * 100.0,
            self.confidence_level()
        )?;
        writeln!(f)?;
        writeln!(f, "Commands:")?;
        writeln!(f, "  Build:  {}", self.build_command)?;
        writeln!(f, "  Test:   {}", self.test_command)?;
        writeln!(f, "  Deploy: {}", self.deploy_command)?;
        if let Some(ref dev_cmd) = self.dev_command {
            writeln!(f, "  Dev:    {}", dev_cmd)?;
        }
        writeln!(f)?;
        writeln!(f, "Reasoning:")?;
        writeln!(f, "  {}", self.reasoning)?;

        if !self.warnings.is_empty() {
            writeln!(f)?;
            writeln!(f, "Warnings:")?;
            for warning in &self.warnings {
                writeln!(f, "  - {}", warning)?;
            }
        }

        if !self.detected_files.is_empty() {
            writeln!(f)?;
            writeln!(f, "Detected Files:")?;
            for file in &self.detected_files {
                writeln!(f, "  - {}", file)?;
            }
        }

        writeln!(f)?;
        writeln!(f, "Processing Time: {}ms", self.processing_time_ms)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_info_creation() {
        let git_info = GitInfo::new("main".to_string(), "abc123".to_string(), SystemTime::now());
        assert_eq!(git_info.branch, "main");
        assert_eq!(git_info.commit_hash, "abc123");
    }

    #[test]
    fn test_repository_context_builder() {
        let context =
            RepositoryContext::minimal(PathBuf::from("/test/repo"), "test/\n└── file".to_string())
                .with_key_file("Cargo.toml".to_string(), "[package]".to_string())
                .with_readme("# Test".to_string());

        assert_eq!(context.key_file_count(), 1);
        assert!(context.has_file("Cargo.toml"));
        assert!(!context.has_file("package.json"));
        assert!(context.readme_content.is_some());
    }

    #[test]
    fn test_detection_result_confidence_levels() {
        let mut result = DetectionResult::new(
            "cargo".to_string(),
            "Rust".to_string(),
            "cargo build".to_string(),
            "cargo test".to_string(),
            "cargo publish".to_string(),
        );

        result.set_confidence(0.95);
        assert_eq!(result.confidence_level(), "Very High");
        assert!(result.is_high_confidence());
        assert!(!result.is_low_confidence());

        result.set_confidence(0.85);
        assert_eq!(result.confidence_level(), "High");

        result.set_confidence(0.75);
        assert_eq!(result.confidence_level(), "Moderate");

        result.set_confidence(0.65);
        assert_eq!(result.confidence_level(), "Low");

        result.set_confidence(0.45);
        assert_eq!(result.confidence_level(), "Very Low");
        assert!(result.is_low_confidence());

        // Test clamping
        result.set_confidence(1.5);
        assert_eq!(result.confidence, 1.0);

        result.set_confidence(-0.5);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_detection_result_warnings() {
        let mut result = DetectionResult::new(
            "npm".to_string(),
            "JavaScript".to_string(),
            "npm run build".to_string(),
            "npm test".to_string(),
            "npm publish".to_string(),
        );

        assert!(!result.has_warnings());
        result.add_warning("No package-lock.json found".to_string());
        assert!(result.has_warnings());
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_detection_result_display() {
        let result = DetectionResult {
            build_system: "cargo".to_string(),
            language: "Rust".to_string(),
            build_command: "cargo build --release".to_string(),
            test_command: "cargo test".to_string(),
            deploy_command: "cargo publish".to_string(),
            dev_command: Some("cargo watch -x run".to_string()),
            confidence: 0.95,
            reasoning: "Standard Rust project with Cargo.toml".to_string(),
            warnings: vec!["Consider adding CI/CD".to_string()],
            detected_files: vec!["Cargo.toml".to_string()],
            processing_time_ms: 1234,
        };

        let display = format!("{}", result);
        assert!(display.contains("cargo"));
        assert!(display.contains("Rust"));
        assert!(display.contains("95.0%"));
        assert!(display.contains("1234ms"));
    }
}
