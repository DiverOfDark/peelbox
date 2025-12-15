// Registry for code-based extractors
use std::path::PathBuf;

pub struct ExtractorRegistry {
    repo_path: PathBuf,
}

impl ExtractorRegistry {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    pub fn repo_path(&self) -> &PathBuf {
        &self.repo_path
    }
}
