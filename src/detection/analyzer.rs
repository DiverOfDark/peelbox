use crate::detection::types::RepositoryContext;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;
use thiserror::Error;
use walkdir::WalkDir;

const DEFAULT_MAX_FILE_SIZE: usize = 50 * 1024;
const DEFAULT_MAX_DEPTH: usize = 3;
const DEFAULT_FILE_TREE_LIMIT: usize = 100;
const MAX_README_SIZE: usize = 5 * 1024;

#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("Path is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Failed to read file {path}: {source}")]
    FileReadError { path: PathBuf, source: io::Error },
    #[error("Repository too large: exceeded file tree limit of {0} entries")]
    TooLarge(usize),
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(String),
    #[error("Analysis error: {0}")]
    Other(String),
}

impl From<regex::Error> for AnalysisError {
    fn from(err: regex::Error) -> Self {
        AnalysisError::InvalidRegex(err.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    pub max_depth: usize,
    pub ignore_patterns: Vec<String>,
    pub max_file_size: usize,
    pub file_tree_limit: usize,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            ignore_patterns: Self::default_ignores(),
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            file_tree_limit: DEFAULT_FILE_TREE_LIMIT,
        }
    }
}

impl AnalyzerConfig {
    pub fn default_ignores() -> Vec<String> {
        vec![
            r"^\.git$".to_string(),
            r"^\.hg$".to_string(),
            r"^\.svn$".to_string(),
            r"^node_modules$".to_string(),
            r"^target$".to_string(),
            r"^dist$".to_string(),
            r"^build$".to_string(),
            r"^out$".to_string(),
            r"^venv$".to_string(),
            r"^\.venv$".to_string(),
            r"^__pycache__$".to_string(),
            r"^\.pytest_cache$".to_string(),
            r"^vendor$".to_string(),
            r"^\.vscode$".to_string(),
            r"^\.idea$".to_string(),
            r"^\.DS_Store$".to_string(),
            r"\.tmp$".to_string(),
            r"\.log$".to_string(),
            r"^coverage$".to_string(),
            r"^\.coverage$".to_string(),
            r"^htmlcov$".to_string(),
        ]
    }

    pub fn add_ignore_pattern(&mut self, pattern: String) {
        self.ignore_patterns.push(pattern);
    }

    fn should_ignore(&self, path: &Path) -> Result<bool, AnalysisError> {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return Ok(false),
        };

        for pattern in &self.ignore_patterns {
            let regex = Regex::new(pattern)?;
            if regex.is_match(&file_name) {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

pub struct RepositoryAnalyzer {
    repo_path: PathBuf,
    config: AnalyzerConfig,
}

impl RepositoryAnalyzer {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo_path,
            config: AnalyzerConfig::default(),
        }
    }

    pub fn with_config(repo_path: PathBuf, config: AnalyzerConfig) -> Self {
        Self { repo_path, config }
    }

    pub async fn analyze(&self) -> Result<RepositoryContext, AnalysisError> {
        let start_time = Instant::now();
        self.validate_repo_path()?;
        let (file_tree, detected_files) = self.walk_filesystem().await?;
        let key_files = self.read_key_files(&detected_files).await?;
        let readme_content = self.find_and_read_readme().await?;
        let context = self.build_context(
            file_tree,
            key_files,
            readme_content,
            detected_files,
            start_time,
        )?;
        Ok(context)
    }

    fn validate_repo_path(&self) -> Result<(), AnalysisError> {
        if !self.repo_path.exists() {
            return Err(AnalysisError::PathNotFound(self.repo_path.clone()));
        }

        if !self.repo_path.is_dir() {
            return Err(AnalysisError::NotADirectory(self.repo_path.clone()));
        }

        Ok(())
    }

    async fn walk_filesystem(&self) -> Result<(String, Vec<PathBuf>), AnalysisError> {
        let mut tree_lines = Vec::new();
        let mut detected_files = Vec::new();
        let mut entry_count = 0;

        let root_name = self
            .repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repository");
        tree_lines.push(format!("{}/", root_name));

        for entry in WalkDir::new(&self.repo_path)
            .max_depth(self.config.max_depth)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                if e.path() == self.repo_path {
                    return true;
                }
                match self.config.should_ignore(e.path()) {
                    Ok(should_ignore) => !should_ignore,
                    Err(_) => true,
                }
            })
        {
            if entry_count >= self.config.file_tree_limit {
                tree_lines.push(format!(
                    "... (truncated at {} entries)",
                    self.config.file_tree_limit
                ));
                return Err(AnalysisError::TooLarge(self.config.file_tree_limit));
            }

            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    if let Some(io_err) = e.io_error() {
                        if io_err.kind() == io::ErrorKind::PermissionDenied {
                            return Err(AnalysisError::PermissionDenied(
                                e.path()
                                    .unwrap_or(Path::new("unknown"))
                                    .display()
                                    .to_string(),
                            ));
                        }
                    }
                    continue;
                }
            };

            if entry.path() == self.repo_path {
                continue;
            }

            entry_count += 1;

            let relative_path = entry
                .path()
                .strip_prefix(&self.repo_path)
                .unwrap_or(entry.path());

            let depth = entry.depth();
            let is_dir = entry.file_type().is_dir();
            let file_name = entry.file_name().to_string_lossy();

            let indent = "  ".repeat(depth.saturating_sub(1));
            let prefix = if depth > 0 { "├── " } else { "" };

            let display_name = if is_dir {
                format!("{}/", file_name)
            } else {
                file_name.to_string()
            };

            tree_lines.push(format!("{}{}{}", indent, prefix, display_name));

            if !is_dir {
                detected_files.push(relative_path.to_path_buf());
            }
        }

        let file_tree = tree_lines.join("\n");
        Ok((file_tree, detected_files))
    }

    fn is_key_file(path: &Path) -> bool {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return false,
        };

        matches!(
            file_name.as_ref(),
            "package.json"
                | ".npmrc"
                | ".yarnrc"
                | "tsconfig.json"
                | "Cargo.toml"
                | "rust-toolchain.toml"
                | "rust-toolchain"
                | "pyproject.toml"
                | "setup.py"
                | "setup.cfg"
                | "requirements.txt"
                | "Pipfile"
                | "tox.ini"
                | "build.gradle"
                | "build.gradle.kts"
                | "pom.xml"
                | "settings.gradle"
                | "settings.gradle.kts"
                | "build.sbt"
                | "project/build.properties"
                | "go.mod"
                | "go.work"
                | "Gemfile"
                | "Rakefile"
                | ".ruby-version"
                | "composer.json"
                | "*.csproj"
                | "*.fsproj"
                | "*.vbproj"
                | "*.sln"
                | "global.json"
                | "nuget.config"
                | "Dockerfile"
                | "docker-compose.yml"
                | "docker-compose.yaml"
                | ".dockerignore"
                | "Makefile"
                | "makefile"
                | "GNUmakefile"
                | ".gitlab-ci.yml"
                | ".travis.yml"
                | "circle.yml"
                | "appveyor.yml"
                | "CMakeLists.txt"
                | "meson.build"
                | "BUILD"
                | "BUILD.bazel"
                | "WORKSPACE"
        ) || file_name.ends_with(".csproj")
            || file_name.ends_with(".fsproj")
            || file_name.ends_with(".vbproj")
            || file_name.ends_with(".sln")
            || (path
                .parent()
                .and_then(|p| p.file_name())
                .map(|p| p == ".github" || p == "workflows")
                .unwrap_or(false)
                && file_name.ends_with(".yml")
                || file_name.ends_with(".yaml"))
    }

    async fn read_key_files(
        &self,
        detected_files: &[PathBuf],
    ) -> Result<HashMap<String, String>, AnalysisError> {
        let mut key_files = HashMap::new();

        for relative_path in detected_files {
            if !Self::is_key_file(relative_path) {
                continue;
            }

            let full_path = self.repo_path.join(relative_path);

            match fs::metadata(&full_path) {
                Ok(metadata) => {
                    if metadata.len() > self.config.max_file_size as u64 {
                        continue;
                    }
                }
                Err(_) => continue,
            }

            match fs::read_to_string(&full_path) {
                Ok(contents) => {
                    let key = relative_path.to_string_lossy().to_string();
                    key_files.insert(key, contents);
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::PermissionDenied {
                        return Err(AnalysisError::PermissionDenied(
                            full_path.display().to_string(),
                        ));
                    }
                    continue;
                }
            }
        }

        Ok(key_files)
    }

    async fn find_and_read_readme(&self) -> Result<Option<String>, AnalysisError> {
        let readme_names = [
            "README.md",
            "README.MD",
            "readme.md",
            "README.txt",
            "README.TXT",
            "readme.txt",
            "README",
            "readme",
            "ReadMe.md",
        ];

        for name in &readme_names {
            let readme_path = self.repo_path.join(name);

            if !readme_path.exists() {
                continue;
            }

            match fs::metadata(&readme_path) {
                Ok(metadata) => {
                    if metadata.len() > MAX_README_SIZE as u64 {
                        match fs::read(&readme_path) {
                            Ok(bytes) => {
                                let truncated = &bytes[..MAX_README_SIZE.min(bytes.len())];
                                if let Ok(content) = String::from_utf8(truncated.to_vec()) {
                                    return Ok(Some(content));
                                }
                            }
                            Err(_) => continue,
                        }
                    } else {
                        match fs::read_to_string(&readme_path) {
                            Ok(content) => return Ok(Some(content)),
                            Err(_) => continue,
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        Ok(None)
    }

    fn build_context(
        &self,
        file_tree: String,
        key_files: HashMap<String, String>,
        readme_content: Option<String>,
        detected_files: Vec<PathBuf>,
        _start_time: Instant,
    ) -> Result<RepositoryContext, AnalysisError> {
        let detected_file_names = detected_files
            .into_iter()
            .filter(|p| Self::is_key_file(p))
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let context = RepositoryContext {
            file_tree,
            key_files,
            readme_content,
            detected_files: detected_file_names,
            repo_path: self.repo_path.clone(),
            git_info: None,
        };

        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create a simple project structure
        fs::create_dir(repo_path.join("src")).unwrap();
        fs::write(repo_path.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
        fs::write(repo_path.join("README.md"), "# Test Project").unwrap();
        fs::write(repo_path.join("src/main.rs"), "fn main() {}").unwrap();

        temp_dir
    }

    #[test]
    fn test_analyzer_config_default() {
        let config = AnalyzerConfig::default();
        assert_eq!(config.max_depth, DEFAULT_MAX_DEPTH);
        assert_eq!(config.max_file_size, DEFAULT_MAX_FILE_SIZE);
        assert_eq!(config.file_tree_limit, DEFAULT_FILE_TREE_LIMIT);
        assert!(!config.ignore_patterns.is_empty());
    }

    #[test]
    fn test_analyzer_config_add_pattern() {
        let mut config = AnalyzerConfig::default();
        let initial_count = config.ignore_patterns.len();

        config.add_ignore_pattern(r"^test_.*".to_string());
        assert_eq!(config.ignore_patterns.len(), initial_count + 1);
    }

    #[test]
    fn test_should_ignore_patterns() {
        let config = AnalyzerConfig::default();

        assert!(config.should_ignore(Path::new("node_modules")).unwrap());
        assert!(config.should_ignore(Path::new(".git")).unwrap());
        assert!(config.should_ignore(Path::new("target")).unwrap());
        assert!(!config.should_ignore(Path::new("src")).unwrap());
        assert!(!config.should_ignore(Path::new("Cargo.toml")).unwrap());
    }

    #[test]
    fn test_is_key_file() {
        assert!(RepositoryAnalyzer::is_key_file(Path::new("Cargo.toml")));
        assert!(RepositoryAnalyzer::is_key_file(Path::new("package.json")));
        assert!(RepositoryAnalyzer::is_key_file(Path::new("go.mod")));
        assert!(RepositoryAnalyzer::is_key_file(Path::new("pom.xml")));
        assert!(RepositoryAnalyzer::is_key_file(Path::new("Dockerfile")));

        assert!(!RepositoryAnalyzer::is_key_file(Path::new("main.rs")));
        assert!(!RepositoryAnalyzer::is_key_file(Path::new("test.txt")));
    }

    #[tokio::test]
    async fn test_validate_repo_path_success() {
        let temp_dir = create_test_repo();
        let analyzer = RepositoryAnalyzer::new(temp_dir.path().to_path_buf());

        assert!(analyzer.validate_repo_path().is_ok());
    }

    #[tokio::test]
    async fn test_validate_repo_path_not_exists() {
        let analyzer = RepositoryAnalyzer::new(PathBuf::from("/nonexistent/path"));

        let result = analyzer.validate_repo_path();
        assert!(matches!(result, Err(AnalysisError::PathNotFound(_))));
    }

    #[tokio::test]
    async fn test_validate_repo_path_not_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "content").unwrap();

        let analyzer = RepositoryAnalyzer::new(file_path);
        let result = analyzer.validate_repo_path();
        assert!(matches!(result, Err(AnalysisError::NotADirectory(_))));
    }

    #[tokio::test]
    async fn test_walk_filesystem_basic() {
        let temp_dir = create_test_repo();
        let analyzer = RepositoryAnalyzer::new(temp_dir.path().to_path_buf());

        let (file_tree, detected_files) = analyzer.walk_filesystem().await.unwrap();

        // Check that file tree contains expected entries
        assert!(file_tree.contains("Cargo.toml"));
        assert!(file_tree.contains("README.md"));
        assert!(file_tree.contains("src/"));

        // Check detected files
        assert!(!detected_files.is_empty());
        assert!(detected_files
            .iter()
            .any(|p| p.to_string_lossy().contains("Cargo.toml")));
    }

    #[tokio::test]
    async fn test_walk_filesystem_respects_depth() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create nested structure
        fs::create_dir(repo_path.join("level1")).unwrap();
        fs::create_dir(repo_path.join("level1/level2")).unwrap();
        fs::create_dir(repo_path.join("level1/level2/level3")).unwrap();
        fs::write(repo_path.join("level1/level2/level3/deep.txt"), "deep").unwrap();

        // Analyze with max_depth = 2
        let config = AnalyzerConfig {
            max_depth: 2,
            ..Default::default()
        };
        let analyzer = RepositoryAnalyzer::with_config(repo_path.to_path_buf(), config);

        let (file_tree, _) = analyzer.walk_filesystem().await.unwrap();

        // Should not contain level3 files
        assert!(!file_tree.contains("deep.txt"));
    }

    #[tokio::test]
    async fn test_walk_filesystem_ignores_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        fs::create_dir(repo_path.join("node_modules")).unwrap();
        fs::write(repo_path.join("node_modules/package.json"), "{}").unwrap();
        fs::write(repo_path.join("main.js"), "console.log('hello')").unwrap();

        let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
        let (file_tree, _) = analyzer.walk_filesystem().await.unwrap();

        // Should contain main.js but not node_modules content
        assert!(file_tree.contains("main.js"));
        // node_modules itself might appear in tree but not its contents
        assert!(!file_tree.contains("node_modules") || !file_tree.contains("package.json"));
    }

    #[tokio::test]
    async fn test_read_key_files() {
        let temp_dir = create_test_repo();
        let analyzer = RepositoryAnalyzer::new(temp_dir.path().to_path_buf());

        let detected_files = vec![
            PathBuf::from("Cargo.toml"),
            PathBuf::from("README.md"),
            PathBuf::from("src/main.rs"),
        ];

        let key_files = analyzer.read_key_files(&detected_files).await.unwrap();

        // Should only include Cargo.toml (README is not a "key file" for build detection)
        assert!(key_files.contains_key("Cargo.toml"));
        assert!(!key_files.contains_key("src/main.rs")); // Not a key file

        // Check content
        assert!(key_files["Cargo.toml"].contains("[package]"));
    }

    #[tokio::test]
    async fn test_read_key_files_respects_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create a large Cargo.toml
        let large_content = "x".repeat(100 * 1024); // 100KB
        fs::write(repo_path.join("Cargo.toml"), large_content).unwrap();

        let config = AnalyzerConfig {
            max_file_size: 50 * 1024,
            ..Default::default()
        };
        let analyzer = RepositoryAnalyzer::with_config(repo_path.to_path_buf(), config);

        let detected_files = vec![PathBuf::from("Cargo.toml")];
        let key_files = analyzer.read_key_files(&detected_files).await.unwrap();

        // Should skip the file due to size
        assert!(!key_files.contains_key("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_find_and_read_readme() {
        let temp_dir = create_test_repo();
        let analyzer = RepositoryAnalyzer::new(temp_dir.path().to_path_buf());

        let readme = analyzer.find_and_read_readme().await.unwrap();

        assert!(readme.is_some());
        assert!(readme.unwrap().contains("# Test Project"));
    }

    #[tokio::test]
    async fn test_find_and_read_readme_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let analyzer = RepositoryAnalyzer::new(temp_dir.path().to_path_buf());

        let readme = analyzer.find_and_read_readme().await.unwrap();
        assert!(readme.is_none());
    }

    #[tokio::test]
    async fn test_find_and_read_readme_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create README with different case
        fs::write(repo_path.join("readme.md"), "# Lowercase README").unwrap();

        let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
        let readme = analyzer.find_and_read_readme().await.unwrap();

        assert!(readme.is_some());
        assert!(readme.unwrap().contains("Lowercase README"));
    }

    #[tokio::test]
    async fn test_analyze_complete_workflow() {
        let temp_dir = create_test_repo();
        let analyzer = RepositoryAnalyzer::new(temp_dir.path().to_path_buf());

        let context = analyzer.analyze().await.unwrap();

        // Verify context is populated
        assert!(!context.file_tree.is_empty());
        assert!(context.file_tree.contains("Cargo.toml"));

        assert!(context.key_files.contains_key("Cargo.toml"));
        assert!(context.readme_content.is_some());

        assert!(!context.detected_files.is_empty());
        assert!(context.detected_files.contains(&"Cargo.toml".to_string()));

        assert_eq!(context.repo_path, temp_dir.path());
    }

    #[tokio::test]
    async fn test_analyze_with_custom_config() {
        let temp_dir = create_test_repo();

        let mut config = AnalyzerConfig::default();
        config.max_depth = 1;
        config.file_tree_limit = 50;

        let analyzer = RepositoryAnalyzer::with_config(temp_dir.path().to_path_buf(), config);
        let context = analyzer.analyze().await.unwrap();

        assert!(!context.file_tree.is_empty());
    }

    #[tokio::test]
    async fn test_analyze_multiple_file_types() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create multiple build system files
        fs::write(repo_path.join("Cargo.toml"), "[package]").unwrap();
        fs::write(repo_path.join("package.json"), "{}").unwrap();
        fs::write(repo_path.join("go.mod"), "module test").unwrap();
        fs::write(repo_path.join("Makefile"), "all:").unwrap();

        let analyzer = RepositoryAnalyzer::new(repo_path.to_path_buf());
        let context = analyzer.analyze().await.unwrap();

        // Should detect all key files
        assert!(context.key_files.contains_key("Cargo.toml"));
        assert!(context.key_files.contains_key("package.json"));
        assert!(context.key_files.contains_key("go.mod"));
        assert!(context.key_files.contains_key("Makefile"));

        assert!(context.detected_files.contains(&"Cargo.toml".to_string()));
        assert!(context.detected_files.contains(&"package.json".to_string()));
    }

    #[tokio::test]
    async fn test_error_display() {
        let err = AnalysisError::PathNotFound(PathBuf::from("/test"));
        assert!(format!("{}", err).contains("/test"));

        let err = AnalysisError::NotADirectory(PathBuf::from("/test/file"));
        assert!(format!("{}", err).contains("not a directory"));

        let err = AnalysisError::PermissionDenied("test".to_string());
        assert!(format!("{}", err).contains("Permission denied"));

        let err = AnalysisError::TooLarge(100);
        assert!(format!("{}", err).contains("100"));
    }
}
