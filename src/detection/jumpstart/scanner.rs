//! Repository scanner for manifest file discovery

use crate::detection::jumpstart::patterns::{
    is_excluded_dir, is_excluded_file, is_manifest_file, MAX_FILES, MAX_SCAN_DEPTH,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

/// A discovered manifest file with metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestFile {
    /// Relative path from repository root
    pub path: String,
    /// File name
    pub name: String,
    /// Directory depth from root (0 = root level)
    pub depth: usize,
}

/// Scanner for discovering manifest files in repositories
pub struct JumpstartScanner {
    repo_path: PathBuf,
    max_depth: usize,
    max_files: usize,
}

impl JumpstartScanner {
    /// Creates a new scanner for the given repository
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        if !repo_path.exists() {
            return Err(anyhow::anyhow!(
                "Repository path does not exist: {:?}",
                repo_path
            ));
        }
        if !repo_path.is_dir() {
            return Err(anyhow::anyhow!(
                "Repository path is not a directory: {:?}",
                repo_path
            ));
        }

        let repo_path = repo_path
            .canonicalize()
            .context("Failed to canonicalize repository path")?;

        debug!(
            repo_path = %repo_path.display(),
            "JumpstartScanner initialized"
        );

        Ok(Self {
            repo_path,
            max_depth: MAX_SCAN_DEPTH,
            max_files: MAX_FILES,
        })
    }

    /// Creates a scanner with custom limits
    pub fn with_limits(repo_path: PathBuf, max_depth: usize, max_files: usize) -> Result<Self> {
        let mut scanner = Self::new(repo_path)?;
        scanner.max_depth = max_depth;
        scanner.max_files = max_files;
        Ok(scanner)
    }

    /// Scans the repository for manifest files
    pub fn scan(&self) -> Result<Vec<ManifestFile>> {
        info!(
            repo = %self.repo_path.display(),
            max_depth = self.max_depth,
            max_files = self.max_files,
            "Starting jumpstart scan"
        );

        let start = std::time::Instant::now();
        let mut manifests = Vec::new();
        let mut files_scanned = 0;

        for entry in WalkDir::new(&self.repo_path)
            .max_depth(self.max_depth)
            .into_iter()
            .filter_entry(|e| self.should_scan_entry(e))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.is_file() {
                files_scanned += 1;

                if files_scanned > self.max_files {
                    warn!(
                        files_scanned,
                        max_files = self.max_files,
                        "Reached file limit, stopping scan"
                    );
                    break;
                }

                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if is_manifest_file(filename) {
                        let rel_path = path
                            .strip_prefix(&self.repo_path)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();

                        let depth = rel_path.split('/').count() - 1;

                        debug!(
                            path = %rel_path,
                            name = filename,
                            depth,
                            "Discovered manifest file"
                        );

                        manifests.push(ManifestFile {
                            path: rel_path,
                            name: filename.to_string(),
                            depth,
                        });
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        info!(
            manifests_found = manifests.len(),
            files_scanned,
            elapsed_ms = elapsed.as_millis(),
            "Jumpstart scan completed"
        );

        Ok(manifests)
    }

    /// Determines if an entry should be scanned
    fn should_scan_entry(&self, entry: &walkdir::DirEntry) -> bool {
        let path = entry.path();

        // Always scan the root
        if path == self.repo_path {
            return true;
        }

        // Skip excluded directories
        if path.is_dir() && is_excluded_dir(path) {
            return false;
        }

        // Skip excluded files
        if path.is_file() && is_excluded_file(path) {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        // Root level manifest
        fs::write(base.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(base.join("package.json"), r#"{"name": "test"}"#).unwrap();

        // Nested manifest
        fs::create_dir(base.join("subproject")).unwrap();
        fs::write(
            base.join("subproject/pom.xml"),
            "<project><artifactId>test</artifactId></project>",
        )
        .unwrap();

        // Excluded directory
        fs::create_dir(base.join("node_modules")).unwrap();
        fs::write(
            base.join("node_modules/package.json"),
            r#"{"name": "ignored"}"#,
        )
        .unwrap();

        // Regular files
        fs::create_dir(base.join("src")).unwrap();
        fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(base.join("README.md"), "# Test").unwrap();

        dir
    }

    #[test]
    fn test_scanner_creation() {
        let temp_dir = create_test_repo();
        let scanner = JumpstartScanner::new(temp_dir.path().to_path_buf());
        assert!(scanner.is_ok());
    }

    #[test]
    fn test_scanner_invalid_path() {
        let scanner = JumpstartScanner::new(PathBuf::from("/nonexistent/path"));
        assert!(scanner.is_err());
    }

    #[test]
    fn test_scan_discovers_manifests() {
        let temp_dir = create_test_repo();
        let scanner = JumpstartScanner::new(temp_dir.path().to_path_buf()).unwrap();

        let manifests = scanner.scan().unwrap();

        assert!(manifests.len() >= 3);

        let manifest_names: Vec<&str> = manifests.iter().map(|m| m.name.as_str()).collect();
        assert!(manifest_names.contains(&"Cargo.toml"));
        assert!(manifest_names.contains(&"package.json"));
        assert!(manifest_names.contains(&"pom.xml"));
    }

    #[test]
    fn test_scan_excludes_node_modules() {
        let temp_dir = create_test_repo();
        let scanner = JumpstartScanner::new(temp_dir.path().to_path_buf()).unwrap();

        let manifests = scanner.scan().unwrap();

        let manifest_paths: Vec<&str> = manifests.iter().map(|m| m.path.as_str()).collect();
        assert!(!manifest_paths.iter().any(|p| p.contains("node_modules")));
    }

    #[test]
    fn test_scan_calculates_depth() {
        let temp_dir = create_test_repo();
        let scanner = JumpstartScanner::new(temp_dir.path().to_path_buf()).unwrap();

        let manifests = scanner.scan().unwrap();

        let cargo_toml = manifests
            .iter()
            .find(|m| m.name == "Cargo.toml")
            .expect("Cargo.toml should be found");
        assert_eq!(cargo_toml.depth, 0);

        let pom_xml = manifests
            .iter()
            .find(|m| m.name == "pom.xml")
            .expect("pom.xml should be found");
        assert_eq!(pom_xml.depth, 1);
    }

    #[test]
    fn test_scan_respects_max_depth() {
        let temp_dir = create_test_repo();
        let scanner =
            JumpstartScanner::with_limits(temp_dir.path().to_path_buf(), 0, 1000).unwrap();

        let manifests = scanner.scan().unwrap();

        // Should only find root-level manifests
        assert!(manifests.iter().all(|m| m.depth == 0));
    }

    #[test]
    fn test_scan_respects_max_files() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create many files
        for i in 0..50 {
            fs::write(base.join(format!("file{}.txt", i)), "content").unwrap();
        }
        fs::write(base.join("Cargo.toml"), "[package]").unwrap();

        let scanner = JumpstartScanner::with_limits(temp_dir.path().to_path_buf(), 10, 10).unwrap();
        let manifests = scanner.scan().unwrap();

        // Should find Cargo.toml if within first 10 files
        assert!(manifests.len() <= 1);
    }
}
