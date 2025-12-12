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
}
