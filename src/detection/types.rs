use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitInfo {
    pub branch: String,
    pub commit_hash: String,
    pub last_modified: SystemTime,
}

impl GitInfo {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryContext {
    pub file_tree: String,
    pub key_files: HashMap<String, String>,
    pub readme_content: Option<String>,
    pub detected_files: Vec<String>,
    pub repo_path: PathBuf,
    pub git_info: Option<GitInfo>,
}

impl RepositoryContext {
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

    pub fn with_key_file(mut self, path: String, content: String) -> Self {
        self.key_files.insert(path.clone(), content);
        if !self.detected_files.contains(&path) {
            self.detected_files.push(path);
        }
        self
    }

    pub fn with_readme(mut self, content: String) -> Self {
        self.readme_content = Some(content);
        self
    }

    pub fn with_git_info(mut self, git_info: GitInfo) -> Self {
        self.git_info = Some(git_info);
        self
    }

    pub fn key_file_count(&self) -> usize {
        self.key_files.len()
    }

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectionResult {
    pub build_system: String,
    pub language: String,
    pub build_command: String,
    pub test_command: String,
    pub runtime: String,
    pub dependencies: Vec<String>,
    pub entry_point: String,
    pub dev_command: Option<String>,
    pub confidence: f32,
    pub reasoning: String,
    pub warnings: Vec<String>,
    pub detected_files: Vec<String>,
    pub processing_time_ms: u64,
}

impl DetectionResult {
    pub fn new(
        build_system: String,
        language: String,
        build_command: String,
        test_command: String,
        runtime: String,
        entry_point: String,
    ) -> Self {
        Self {
            build_system,
            language,
            build_command,
            test_command,
            runtime,
            dependencies: Vec::new(),
            entry_point,
            dev_command: None,
            confidence: 0.8,
            reasoning: String::new(),
            warnings: Vec::new(),
            detected_files: Vec::new(),
            processing_time_ms: 0,
        }
    }

    pub fn is_high_confidence(&self) -> bool {
        self.confidence >= 0.8
    }

    pub fn is_low_confidence(&self) -> bool {
        self.confidence < 0.6
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    pub fn set_confidence(&mut self, confidence: f32) {
        self.confidence = confidence.clamp(0.0, 1.0);
    }

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
        writeln!(f, "Docker Build Detection Result")?;
        writeln!(f, "==============================")?;
        writeln!(f, "Language: {}", self.language)?;
        writeln!(f, "Build System: {}", self.build_system)?;
        writeln!(
            f,
            "Confidence: {:.1}% ({})",
            self.confidence * 100.0,
            self.confidence_level()
        )?;
        writeln!(f)?;
        writeln!(f, "Build Information:")?;
        writeln!(f, "  Build:   {}", self.build_command)?;
        writeln!(f, "  Test:    {}", self.test_command)?;
        if let Some(ref dev_cmd) = self.dev_command {
            writeln!(f, "  Dev:     {}", dev_cmd)?;
        }
        writeln!(f)?;
        writeln!(f, "Docker Information:")?;
        writeln!(f, "  Runtime:     {}", self.runtime)?;
        writeln!(f, "  Entry Point: {}", self.entry_point)?;
        if !self.dependencies.is_empty() {
            writeln!(f, "  Dependencies:")?;
            for dep in &self.dependencies {
                writeln!(f, "    - {}", dep)?;
            }
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
            "rust:1.75".to_string(),
            "/app/target/release/myapp".to_string(),
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
            "node:20".to_string(),
            "node index.js".to_string(),
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
            runtime: "rust:1.75".to_string(),
            dependencies: vec!["ca-certificates".to_string()],
            entry_point: "/app/target/release/myapp".to_string(),
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
