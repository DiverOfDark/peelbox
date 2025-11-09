use anyhow::{anyhow, Context, Result};
use glob::Pattern;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::output::UniversalBuild;

#[derive(Serialize)]
struct TreeNode {
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<TreeNode>>,
}

const MAX_FILE_SIZE: u64 = 1024 * 1024;
const DEFAULT_MAX_LINES: usize = 500;
const DEFAULT_MAX_RESULTS: usize = 20;
const DEFAULT_MAX_MATCHES: usize = 10;
const DEFAULT_TREE_DEPTH: usize = 2;

pub struct ToolExecutor {
    repo_path: PathBuf,
}

impl ToolExecutor {
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        if !repo_path.exists() {
            return Err(anyhow!("Repository path does not exist: {:?}", repo_path));
        }
        if !repo_path.is_dir() {
            return Err(anyhow!(
                "Repository path is not a directory: {:?}",
                repo_path
            ));
        }

        // Canonicalize the repo path to ensure consistent path comparisons
        let repo_path = repo_path
            .canonicalize()
            .context("Failed to canonicalize repository path")?;

        debug!(repo_path = %repo_path.display(), "ToolExecutor initialized");
        Ok(Self { repo_path })
    }

    pub async fn execute(&self, tool_name: &str, arguments: Value) -> Result<String> {
        info!(tool = tool_name, args = ?arguments, "Executing tool");

        let result = match tool_name {
            "list_files" => self.list_files(arguments).await,
            "read_file" => self.read_file(arguments).await,
            "search_files" => self.search_files(arguments).await,
            "get_file_tree" => self.get_file_tree(arguments).await,
            "grep_content" => self.grep_content(arguments).await,
            "submit_detection" => self.submit_detection(arguments).await,
            _ => {
                warn!(tool = tool_name, "Unknown tool requested");
                Err(anyhow!("Unknown tool: {}", tool_name))
            }
        };

        match &result {
            Ok(output) => {
                let output_len = output.len();
                info!(tool = tool_name, output_len, "Tool execution completed");
                debug!(
                    tool = tool_name,
                    output_preview = &output[..output.len().min(200)],
                    "Tool output preview"
                );
            }
            Err(e) => {
                warn!(tool = tool_name, error = %e, "Tool execution failed");
            }
        }

        result
    }

    async fn list_files(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or(".").trim_start_matches('/');
        let pattern = args["pattern"].as_str();
        let max_depth = args["max_depth"].as_u64().map(|d| d as usize);

        debug!(path, pattern, max_depth, "list_files parameters");

        let target_path = self.validate_path(path)?;

        let mut walker = WalkDir::new(&target_path);
        if let Some(depth) = max_depth {
            walker = walker.max_depth(depth);
        }

        let mut files = Vec::new();
        for entry in walker
            .into_iter()
            .filter_entry(|e| !self.is_ignored(e.path()))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.is_file() {
                let rel_path = path
                    .strip_prefix(&self.repo_path)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                if let Some(pat) = pattern {
                    if Pattern::new(pat)
                        .context("Invalid glob pattern")?
                        .matches(&rel_path)
                    {
                        files.push(rel_path);
                    }
                } else {
                    files.push(rel_path);
                }
            }
        }

        debug!(files_found = files.len(), "list_files completed");
        Ok(files.join("\n"))
    }

    async fn read_file(&self, args: Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?
            .trim_start_matches('/');
        let max_lines = args["max_lines"]
            .as_u64()
            .map(|l| l as usize)
            .unwrap_or(DEFAULT_MAX_LINES);

        debug!(path, max_lines, "read_file parameters");

        let file_path = self.validate_path(path)?;

        let metadata = fs::metadata(&file_path)
            .context(format!("Failed to read file metadata: {:?}", file_path))?;

        if metadata.len() > MAX_FILE_SIZE {
            warn!(
                path,
                file_size = metadata.len(),
                max_size = MAX_FILE_SIZE,
                "File too large to read"
            );
            return Err(anyhow!(
                "File too large: {} bytes (max {} bytes)",
                metadata.len(),
                MAX_FILE_SIZE
            ));
        }

        if self.is_binary(&file_path)? {
            warn!(path, "Cannot read binary file");
            return Err(anyhow!("Cannot read binary file: {:?}", path));
        }

        debug!(path, file_size = metadata.len(), "Reading file");

        let content = fs::read_to_string(&file_path)
            .context(format!("Failed to read file: {:?}", file_path))?;

        let lines: Vec<&str> = content.lines().collect();
        let truncated_lines: Vec<&str> = lines.iter().take(max_lines).copied().collect();

        let mut result = truncated_lines.join("\n");
        if lines.len() > max_lines {
            debug!(
                path,
                total_lines = lines.len(),
                returned_lines = max_lines,
                "File content truncated"
            );
            result.push_str(&format!(
                "\n... (truncated {} lines)",
                lines.len() - max_lines
            ));
        }

        Ok(result)
    }

    async fn search_files(&self, args: Value) -> Result<String> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'pattern' parameter"))?;
        let max_results = args["max_results"]
            .as_u64()
            .map(|r| r as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS);

        debug!(pattern, max_results, "search_files parameters");

        let glob_pattern = Pattern::new(pattern).context("Invalid glob pattern")?;
        let mut matches = Vec::new();

        for entry in WalkDir::new(&self.repo_path)
            .into_iter()
            .filter_entry(|e| !self.is_ignored(e.path()))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.is_file() {
                let rel_path = path
                    .strip_prefix(&self.repo_path)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                if glob_pattern.matches(&rel_path) {
                    matches.push(rel_path);
                    if matches.len() >= max_results {
                        break;
                    }
                }
            }
        }

        debug!(matches_found = matches.len(), "search_files completed");

        if matches.is_empty() {
            Ok(format!("No files found matching pattern: {}", pattern))
        } else {
            Ok(matches.join("\n"))
        }
    }

    async fn get_file_tree(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or(".").trim_start_matches('/');
        let depth = args["depth"]
            .as_u64()
            .map(|d| d as usize)
            .unwrap_or(DEFAULT_TREE_DEPTH);

        debug!(path, depth, "get_file_tree parameters");

        let target_path = self.validate_path(path)?;

        let tree_json = self.build_tree_json(&target_path, 0, depth)?;

        serde_json::to_string_pretty(&tree_json).context("Failed to serialize file tree to JSON")
    }

    async fn grep_content(&self, args: Value) -> Result<String> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'pattern' parameter"))?;
        let file_pattern = args["file_pattern"].as_str();
        let max_matches = args["max_matches"]
            .as_u64()
            .map(|m| m as usize)
            .unwrap_or(DEFAULT_MAX_MATCHES);

        debug!(
            pattern,
            file_pattern, max_matches, "grep_content parameters"
        );

        let regex = Regex::new(pattern).context("Invalid regex pattern")?;
        let file_glob = file_pattern.map(Pattern::new).transpose()?;

        let mut matches = Vec::new();
        let mut match_count = 0;

        for entry in WalkDir::new(&self.repo_path)
            .into_iter()
            .filter_entry(|e| !self.is_ignored(e.path()))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let rel_path = path
                .strip_prefix(&self.repo_path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            if let Some(ref glob) = file_glob {
                if !glob.matches(&rel_path) {
                    continue;
                }
            }

            if self.is_binary(path).unwrap_or(true) {
                continue;
            }

            let metadata = match fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            if metadata.len() > MAX_FILE_SIZE {
                continue;
            }

            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for (line_num, line) in content.lines().enumerate() {
                if regex.is_match(line) {
                    matches.push(format!("{}:{}: {}", rel_path, line_num + 1, line));
                    match_count += 1;
                    if match_count >= max_matches {
                        return Ok(matches.join("\n"));
                    }
                }
            }
        }

        debug!(
            matches_found = matches.len(),
            files_searched = match_count,
            "grep_content completed"
        );

        if matches.is_empty() {
            Ok(format!("No matches found for pattern: {}", pattern))
        } else {
            Ok(matches.join("\n"))
        }
    }

    async fn submit_detection(&self, args: Value) -> Result<String> {
        info!("LLM submitting final UniversalBuild detection result");
        debug!(universal_build = ?args, "UniversalBuild submission");

        let universal_build: UniversalBuild = serde_json::from_value(args)
            .context("Failed to parse UniversalBuild from LLM response")?;

        universal_build
            .validate()
            .context("UniversalBuild validation failed")?;

        info!(
            language = %universal_build.metadata.language,
            build_system = %universal_build.metadata.build_system,
            confidence = %universal_build.metadata.confidence,
            "UniversalBuild validated successfully"
        );

        serde_json::to_string_pretty(&universal_build)
            .context("Failed to serialize UniversalBuild")
    }

    fn validate_path(&self, path: &str) -> Result<PathBuf> {
        let normalized = path.trim_start_matches('/');
        let full_path = self.repo_path.join(normalized);

        // Check if path exists before canonicalize
        if !full_path.exists() {
            return Err(anyhow!("File or directory does not exist: {}", path));
        }

        let canonical = full_path
            .canonicalize()
            .context(format!("Failed to resolve path: {:?}", full_path))?;

        debug!(
            requested_path = path,
            normalized = normalized,
            repo_path = %self.repo_path.display(),
            canonical = %canonical.display(),
            "Validating path"
        );

        if !canonical.starts_with(&self.repo_path) {
            warn!(
                requested_path = path,
                canonical = %canonical.display(),
                repo_path = %self.repo_path.display(),
                "Path traversal attempt detected"
            );
            return Err(anyhow!(
                "Path traversal detected: {:?} is outside repository",
                path
            ));
        }

        Ok(canonical)
    }

    fn is_ignored(&self, path: &Path) -> bool {
        const IGNORED_DIRS: &[&str] = &[
            ".git",
            "node_modules",
            "target",
            "dist",
            "build",
            "out",
            "venv",
            ".venv",
            "__pycache__",
            ".pytest_cache",
            "vendor",
            ".idea",
            ".vscode",
            "coverage",
            ".coverage",
            "htmlcov",
        ];

        const IGNORED_FILES: &[&str] = &[".DS_Store"];

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if IGNORED_DIRS.contains(&name) || IGNORED_FILES.contains(&name) {
                return true;
            }
            if name.ends_with(".tmp") || name.ends_with(".log") {
                return true;
            }
        }

        false
    }

    fn is_binary(&self, path: &Path) -> Result<bool> {
        let mut file = fs::File::open(path)?;
        let mut buffer = [0u8; 512];
        let bytes_read = file.read(&mut buffer)?;

        Ok(buffer[..bytes_read].contains(&0))
    }

    fn build_tree_json(
        &self,
        path: &Path,
        current_depth: usize,
        max_depth: usize,
    ) -> Result<TreeNode> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(".")
            .to_string();

        let is_dir = path.is_dir();
        let node_type = if is_dir { "directory" } else { "file" }.to_string();

        let children = if is_dir && current_depth < max_depth {
            let entries: Result<Vec<_>, _> = fs::read_dir(path)?
                .filter(|e| {
                    if let Ok(entry) = e {
                        !self.is_ignored(&entry.path())
                    } else {
                        true
                    }
                })
                .collect();

            let mut entries = entries?;
            entries.sort_by_key(|e| e.file_name());

            let child_nodes: Result<Vec<TreeNode>> = entries
                .iter()
                .map(|entry| self.build_tree_json(&entry.path(), current_depth + 1, max_depth))
                .collect();

            Some(child_nodes?)
        } else {
            None
        };

        Ok(TreeNode {
            name,
            node_type,
            children,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        fs::create_dir(base.join("src")).unwrap();
        File::create(base.join("src/main.rs"))
            .unwrap()
            .write_all(b"fn main() {\n    println!(\"Hello\");\n}")
            .unwrap();

        File::create(base.join("Cargo.toml"))
            .unwrap()
            .write_all(b"[package]\nname = \"test\"\n")
            .unwrap();

        File::create(base.join("README.md"))
            .unwrap()
            .write_all(b"# Test Project\n")
            .unwrap();

        fs::create_dir(base.join("node_modules")).unwrap();
        File::create(base.join("node_modules/test.js"))
            .unwrap()
            .write_all(b"// ignored")
            .unwrap();

        dir
    }

    #[tokio::test]
    async fn test_list_files() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .list_files(json!({
                "path": ".",
                "max_depth": 3
            }))
            .await
            .unwrap();

        assert!(result.contains("Cargo.toml"));
        assert!(result.contains("src/main.rs"));
        assert!(!result.contains("node_modules"));
    }

    #[tokio::test]
    async fn test_list_files_with_pattern() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .list_files(json!({
                "path": ".",
                "pattern": "*.rs"
            }))
            .await
            .unwrap();

        assert!(result.contains("src/main.rs"));
        assert!(!result.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_read_file() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .read_file(json!({
                "path": "src/main.rs"
            }))
            .await
            .unwrap();

        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
    }

    #[tokio::test]
    async fn test_read_file_with_max_lines() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .read_file(json!({
                "path": "src/main.rs",
                "max_lines": 1
            }))
            .await
            .unwrap();

        assert!(result.contains("fn main()"));
        assert!(result.contains("truncated"));
    }

    #[tokio::test]
    async fn test_search_files() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .search_files(json!({
                "pattern": "*.toml"
            }))
            .await
            .unwrap();

        assert!(result.contains("Cargo.toml"));
    }

    #[tokio::test]
    async fn test_get_file_tree() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .get_file_tree(json!({
                "path": ".",
                "depth": 2
            }))
            .await
            .unwrap();

        // Parse JSON result
        let tree: serde_json::Value = serde_json::from_str(&result).unwrap();

        // Check root structure
        assert_eq!(tree["type"], "directory");

        // Convert to string for pattern matching
        let json_str = serde_json::to_string(&tree).unwrap();

        // Check that expected files/directories are in the JSON
        assert!(json_str.contains("Cargo.toml"));
        assert!(json_str.contains("src"));
        assert!(json_str.contains("main.rs"));
        assert!(!json_str.contains("node_modules"));
    }

    #[tokio::test]
    async fn test_grep_content() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .grep_content(json!({
                "pattern": "fn main"
            }))
            .await
            .unwrap();

        assert!(result.contains("src/main.rs"));
        assert!(result.contains("fn main()"));
    }

    #[tokio::test]
    async fn test_grep_content_with_file_pattern() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .grep_content(json!({
                "pattern": "name",
                "file_pattern": "*.toml"
            }))
            .await
            .unwrap();

        assert!(result.contains("Cargo.toml"));
        assert!(result.contains("name"));
    }

    #[tokio::test]
    async fn test_submit_detection() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let universal_build = json!({
            "version": "1.0",
            "metadata": {
                "project_name": "test",
                "language": "rust",
                "build_system": "cargo",
                "confidence": 0.95,
                "reasoning": "Detected Cargo.toml with standard Rust project structure"
            },
            "build": {
                "base": "rust:1.75",
                "packages": [],
                "env": {},
                "commands": ["cargo build --release"],
                "context": [{"from": ".", "to": "/app"}],
                "cache": ["/usr/local/cargo/registry"],
                "artifacts": ["target/release/test"]
            },
            "runtime": {
                "base": "debian:bookworm-slim",
                "packages": ["ca-certificates"],
                "env": {},
                "copy": [
                    {
                        "from": "target/release/test",
                        "to": "/usr/local/bin/test"
                    }
                ],
                "command": ["/usr/local/bin/test"],
                "ports": [],
                "healthcheck": null
            }
        });

        let result = executor.submit_detection(universal_build.clone()).await.unwrap();

        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["version"], "1.0");
        assert_eq!(parsed["metadata"]["language"], "rust");
        assert_eq!(parsed["metadata"]["build_system"], "cargo");
        assert_eq!(parsed["metadata"]["confidence"], 0.95);
        assert_eq!(parsed["build"]["base"], "rust:1.75");
        assert_eq!(parsed["runtime"]["base"], "debian:bookworm-slim");
    }

    #[tokio::test]
    async fn test_path_traversal_protection() {
        let temp_dir = create_test_repo();
        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .read_file(json!({
                "path": "../../../etc/passwd"
            }))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_binary_file_detection() {
        let temp_dir = TempDir::new().unwrap();
        let binary_file = temp_dir.path().join("binary.bin");
        File::create(&binary_file)
            .unwrap()
            .write_all(&[0x00, 0x01, 0x02, 0xFF])
            .unwrap();

        let executor = ToolExecutor::new(temp_dir.path().to_path_buf()).unwrap();

        let result = executor
            .read_file(json!({
                "path": "binary.bin"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("binary"));
    }
}
