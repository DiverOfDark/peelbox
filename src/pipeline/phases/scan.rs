use crate::bootstrap::{BootstrapContext, BootstrapScanner};
use crate::languages::LanguageRegistry;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub repo_path: PathBuf,
    pub bootstrap_context: BootstrapContext,
    pub file_tree: Vec<PathBuf>,
}

impl ScanResult {
    pub fn get_files_in_dir(&self, dir: &Path) -> Vec<PathBuf> {
        self.file_tree
            .iter()
            .filter(|p| p.starts_with(dir))
            .cloned()
            .collect()
    }

    pub fn find_files_by_name(&self, filename: &str) -> Vec<PathBuf> {
        self.file_tree
            .iter()
            .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some(filename))
            .cloned()
            .collect()
    }
}

pub fn execute(repo_path: &Path) -> Result<ScanResult> {
    let repo_path = repo_path.to_path_buf();

    let scanner = BootstrapScanner::new(repo_path.clone())
        .context("Failed to create BootstrapScanner")?;

    let bootstrap_context = scanner.scan().context("Failed to scan repository")?;

    let file_tree = collect_file_tree(&repo_path, Arc::new(LanguageRegistry::with_defaults()))?;

    Ok(ScanResult {
        repo_path,
        bootstrap_context,
        file_tree,
    })
}

fn collect_file_tree(repo_path: &Path, registry: Arc<LanguageRegistry>) -> Result<Vec<PathBuf>> {
    let excluded_dirs = registry.all_excluded_dirs();

    let mut files = Vec::new();

    for entry in WalkDir::new(repo_path)
        .max_depth(10)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), repo_path, &excluded_dirs))
    {
        let entry = entry.context("Failed to read directory entry")?;
        if entry.file_type().is_file() {
            let rel_path = entry
                .path()
                .strip_prefix(repo_path)
                .unwrap_or(entry.path())
                .to_path_buf();
            files.push(rel_path);
        }
    }

    Ok(files)
}

fn is_excluded(path: &Path, repo_path: &Path, excluded_dirs: &[&str]) -> bool {
    if path == repo_path {
        return false;
    }

    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if excluded_dirs.contains(&name) {
            return true;
        }
        if path.is_dir() && name.starts_with('.') && name.len() > 1 {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        fs::write(
            base.join("Cargo.toml"),
            b"[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        fs::create_dir_all(base.join("src")).unwrap();
        fs::write(base.join("src/main.rs"), b"fn main() {}").unwrap();

        fs::create_dir_all(base.join("node_modules")).unwrap();
        fs::write(base.join("node_modules/ignored.js"), b"// ignored").unwrap();

        dir
    }

    #[test]
    fn test_scan_execution() {
        let temp_dir = create_test_repo();
        let result = execute(temp_dir.path());

        assert!(result.is_ok());
        let scan = result.unwrap();

        assert!(!scan.bootstrap_context.detections.is_empty());
        assert!(!scan.file_tree.is_empty());
    }

    #[test]
    fn test_file_tree_excludes_node_modules() {
        let temp_dir = create_test_repo();
        let scan = execute(temp_dir.path()).unwrap();

        let has_node_modules = scan
            .file_tree
            .iter()
            .any(|p| p.to_string_lossy().contains("node_modules"));

        assert!(!has_node_modules);
    }

    #[test]
    fn test_find_files_by_name() {
        let temp_dir = create_test_repo();
        let scan = execute(temp_dir.path()).unwrap();

        let cargo_tomls = scan.find_files_by_name("Cargo.toml");
        assert_eq!(cargo_tomls.len(), 1);
    }
}
