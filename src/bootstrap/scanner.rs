//! Bootstrap scanner using LanguageRegistry for detection

use super::{BootstrapContext, LanguageDetection};
use crate::languages::LanguageRegistry;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

/// Configuration for bootstrap scanning
#[derive(Debug, Clone)]
pub struct ScanConfig {
    /// Maximum directory depth to scan
    pub max_depth: usize,
    /// Maximum number of files to scan
    pub max_files: usize,
    /// Read manifest content for better detection
    pub read_content: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_depth: 10,
            max_files: 1000,
            read_content: true,
        }
    }
}

/// Bootstrap scanner that uses LanguageRegistry for detection
pub struct BootstrapScanner {
    repo_path: PathBuf,
    registry: Arc<LanguageRegistry>,
    config: ScanConfig,
    gitignore_dirs: Vec<String>,
}

impl BootstrapScanner {
    /// Creates a new scanner with default language registry
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        Self::with_registry(repo_path, Arc::new(LanguageRegistry::with_defaults()))
    }

    /// Creates a scanner with a custom language registry
    pub fn with_registry(repo_path: PathBuf, registry: Arc<LanguageRegistry>) -> Result<Self> {
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

        let gitignore_dirs = Self::parse_gitignore(&repo_path);

        debug!(
            repo_path = %repo_path.display(),
            gitignore_entries = gitignore_dirs.len(),
            "BootstrapScanner initialized"
        );

        Ok(Self {
            repo_path,
            registry,
            config: ScanConfig::default(),
            gitignore_dirs,
        })
    }

    /// Parse .gitignore file and extract directory patterns
    fn parse_gitignore(repo_path: &Path) -> Vec<String> {
        let gitignore_path = repo_path.join(".gitignore");
        if !gitignore_path.exists() {
            return Vec::new();
        }

        let content = match std::fs::read_to_string(&gitignore_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                // Skip empty lines and comments
                if line.is_empty() || line.starts_with('#') {
                    return None;
                }
                // Extract directory patterns (ending with / or simple names that are likely dirs)
                let pattern = line.trim_start_matches('/').trim_end_matches('/');
                // Skip patterns with wildcards or complex patterns
                if pattern.contains('*') || pattern.contains('?') || pattern.contains('[') {
                    return None;
                }
                // Only include simple directory names
                if !pattern.contains('/') && !pattern.is_empty() {
                    Some(pattern.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Configure scanning options
    pub fn with_config(mut self, config: ScanConfig) -> Self {
        self.config = config;
        self
    }

    /// Scans the repository and returns bootstrap context
    pub fn scan(&self) -> Result<BootstrapContext> {
        let start = Instant::now();

        info!(
            repo = %self.repo_path.display(),
            max_depth = self.config.max_depth,
            max_files = self.config.max_files,
            "Starting bootstrap scan"
        );

        let mut detections = Vec::new();
        let mut files_scanned = 0;
        let mut has_workspace_config = false;

        for entry in WalkDir::new(&self.repo_path)
            .max_depth(self.config.max_depth)
            .into_iter()
            .filter_entry(|e| !self.is_excluded(e.path()))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            files_scanned += 1;
            if files_scanned > self.config.max_files {
                warn!(
                    files_scanned,
                    max_files = self.config.max_files,
                    "Reached file limit, stopping scan"
                );
                break;
            }

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                // Check for workspace configuration files
                if self.is_workspace_config(filename) {
                    has_workspace_config = true;
                }

                // Check if this is a known manifest file
                if self.registry.is_manifest(filename) {
                    if let Some(detection) = self.detect_language(path, filename)? {
                        debug!(
                            path = %path.display(),
                            language = %detection.language,
                            build_system = %detection.build_system,
                            confidence = detection.confidence,
                            "Detected language"
                        );
                        detections.push(detection);
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        let scan_time_ms = elapsed.as_millis() as u64;

        info!(
            detections_found = detections.len(),
            files_scanned, scan_time_ms, "Bootstrap scan completed"
        );

        Ok(BootstrapContext::from_detections(
            detections,
            has_workspace_config,
            scan_time_ms,
        ))
    }

    /// Detects language for a manifest file
    fn detect_language(&self, path: &Path, filename: &str) -> Result<Option<LanguageDetection>> {
        let content = if self.config.read_content {
            std::fs::read_to_string(path).ok()
        } else {
            None
        };

        let rel_path = path
            .strip_prefix(&self.repo_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let depth = rel_path.matches('/').count();

        if let Some(detection) = self.registry.detect(filename, content.as_deref()) {
            Ok(Some(LanguageDetection {
                language: detection.language,
                build_system: detection.build_system,
                manifest_path: rel_path,
                depth,
                confidence: detection.confidence,
            }))
        } else {
            Ok(None)
        }
    }

    /// Checks if a path should be excluded from scanning
    fn is_excluded(&self, path: &Path) -> bool {
        // Never exclude the root repo path itself
        if path == self.repo_path {
            return false;
        }

        let excluded_dirs = self.registry.all_excluded_dirs();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            // Check registry-provided excluded dirs
            if excluded_dirs.contains(&name) {
                return true;
            }
            // Check .gitignore patterns
            if self.gitignore_dirs.iter().any(|d| d == name) {
                return true;
            }
            // Exclude hidden directories (but not files)
            if path.is_dir() && name.starts_with('.') && name.len() > 1 {
                return true;
            }
        }

        false
    }

    /// Checks if a file is a workspace configuration
    fn is_workspace_config(&self, filename: &str) -> bool {
        self.registry.all_workspace_configs().contains(&filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        // Root level Cargo.toml
        fs::File::create(base.join("Cargo.toml"))
            .unwrap()
            .write_all(b"[package]\nname = \"test\"\nversion = \"0.1.0\"")
            .unwrap();

        // Root level package.json
        fs::File::create(base.join("package.json"))
            .unwrap()
            .write_all(b"{\"name\": \"test\", \"version\": \"1.0.0\"}")
            .unwrap();

        // Nested project
        fs::create_dir_all(base.join("crates/lib")).unwrap();
        fs::File::create(base.join("crates/lib/Cargo.toml"))
            .unwrap()
            .write_all(b"[package]\nname = \"lib\"")
            .unwrap();

        // Ignored directory
        fs::create_dir(base.join("node_modules")).unwrap();
        fs::File::create(base.join("node_modules/package.json"))
            .unwrap()
            .write_all(b"{\"name\": \"ignored\"}")
            .unwrap();

        dir
    }

    #[test]
    fn test_scanner_creation() {
        let temp_dir = create_test_repo();
        let scanner = BootstrapScanner::new(temp_dir.path().to_path_buf());
        assert!(scanner.is_ok());
    }

    #[test]
    fn test_scanner_invalid_path() {
        let scanner = BootstrapScanner::new(PathBuf::from("/nonexistent/path"));
        assert!(scanner.is_err());
    }

    #[test]
    fn test_scan_detects_languages() {
        let temp_dir = create_test_repo();
        let scanner = BootstrapScanner::new(temp_dir.path().to_path_buf()).unwrap();

        let context = scanner.scan().unwrap();

        assert!(context.detections.len() >= 2);

        let languages: Vec<&str> = context
            .detections
            .iter()
            .map(|d| d.language.as_str())
            .collect();
        assert!(languages.contains(&"Rust"));
        assert!(languages.contains(&"JavaScript"));
    }

    #[test]
    fn test_scan_excludes_node_modules() {
        let temp_dir = create_test_repo();
        let scanner = BootstrapScanner::new(temp_dir.path().to_path_buf()).unwrap();

        let context = scanner.scan().unwrap();

        let paths: Vec<&str> = context
            .detections
            .iter()
            .map(|d| d.manifest_path.as_str())
            .collect();

        assert!(!paths.iter().any(|p| p.contains("node_modules")));
    }

    #[test]
    fn test_scan_detects_depth() {
        let temp_dir = create_test_repo();
        let scanner = BootstrapScanner::new(temp_dir.path().to_path_buf()).unwrap();

        let context = scanner.scan().unwrap();

        let root_detections: Vec<_> = context.detections.iter().filter(|d| d.depth == 0).collect();
        let nested_detections: Vec<_> = context.detections.iter().filter(|d| d.depth > 0).collect();

        assert!(!root_detections.is_empty());
        assert!(!nested_detections.is_empty());
    }

    #[test]
    fn test_scan_with_workspace_config() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        fs::File::create(base.join("package.json"))
            .unwrap()
            .write_all(b"{\"name\": \"root\"}")
            .unwrap();
        fs::File::create(base.join("pnpm-workspace.yaml"))
            .unwrap()
            .write_all(b"packages:\n  - packages/*")
            .unwrap();

        let scanner = BootstrapScanner::new(temp_dir.path().to_path_buf()).unwrap();
        let context = scanner.scan().unwrap();

        assert!(context.workspace.has_workspace_config);
        assert!(context.summary.is_monorepo);
    }

    #[test]
    fn test_scan_config() {
        let temp_dir = create_test_repo();
        let config = ScanConfig {
            max_depth: 1,
            max_files: 100,
            read_content: false,
        };

        let scanner = BootstrapScanner::new(temp_dir.path().to_path_buf())
            .unwrap()
            .with_config(config);

        let context = scanner.scan().unwrap();

        // With max_depth=1, should only find root-level manifests
        let nested = context.detections.iter().filter(|d| d.depth > 0).count();
        assert_eq!(nested, 0);
    }

    #[test]
    fn test_format_for_prompt() {
        let temp_dir = create_test_repo();
        let scanner = BootstrapScanner::new(temp_dir.path().to_path_buf()).unwrap();

        let context = scanner.scan().unwrap();
        let prompt = context.format_for_prompt();

        assert!(prompt.contains("Pre-scanned Repository"));
        assert!(prompt.contains("Rust"));
        assert!(prompt.contains("cargo"));
    }

    #[test]
    fn test_gitignore_exclusion() {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        // Create .gitignore with custom_build directory
        fs::write(
            base.join(".gitignore"),
            "custom_build/\n# comment\nsome_cache\n",
        )
        .unwrap();

        // Create root package.json
        fs::File::create(base.join("package.json"))
            .unwrap()
            .write_all(b"{\"name\": \"root\"}")
            .unwrap();

        // Create a package.json in gitignored directory
        fs::create_dir(base.join("custom_build")).unwrap();
        fs::File::create(base.join("custom_build/package.json"))
            .unwrap()
            .write_all(b"{\"name\": \"ignored\"}")
            .unwrap();

        let scanner = BootstrapScanner::new(base.to_path_buf()).unwrap();
        let context = scanner.scan().unwrap();

        // Should only find root package.json, not the one in custom_build
        assert_eq!(context.detections.len(), 1);
        assert_eq!(context.detections[0].manifest_path, "package.json");
    }
}
