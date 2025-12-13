use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use glob::Pattern;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use super::trait_def::Tool;
use crate::languages::LanguageRegistry;
use crate::output::UniversalBuild;

const MAX_FILE_SIZE: u64 = 1024 * 1024;
const DEFAULT_MAX_LINES: usize = 500;
const DEFAULT_MAX_RESULTS: usize = 20;
const DEFAULT_MAX_MATCHES: usize = 10;
const DEFAULT_TREE_DEPTH: usize = 2;

#[derive(Serialize)]
struct TreeNode {
    name: String,
    #[serde(rename = "type")]
    node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<TreeNode>>,
}

pub struct ToolHelpers {
    repo_path: PathBuf,
    language_registry: Arc<LanguageRegistry>,
}

impl ToolHelpers {
    pub fn new(repo_path: PathBuf, language_registry: Arc<LanguageRegistry>) -> Result<Self> {
        if !repo_path.exists() {
            return Err(anyhow!("Repository path does not exist: {:?}", repo_path));
        }
        if !repo_path.is_dir() {
            return Err(anyhow!(
                "Repository path is not a directory: {:?}",
                repo_path
            ));
        }

        let repo_path = repo_path
            .canonicalize()
            .context("Failed to canonicalize repository path")?;

        Ok(Self {
            repo_path,
            language_registry,
        })
    }

    pub fn validate_path(&self, path: &str) -> Result<PathBuf> {
        let normalized = path.trim_start_matches('/');
        let full_path = self.repo_path.join(normalized);

        if !full_path.exists() {
            return Err(anyhow!("File or directory does not exist: {}", path));
        }

        let canonical = full_path
            .canonicalize()
            .context(format!("Failed to resolve path: {:?}", full_path))?;

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

    pub fn is_ignored(&self, path: &Path) -> bool {
        const IGNORED_FILES: &[&str] = &[".DS_Store"];

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let excluded_dirs = self.language_registry.all_excluded_dirs();
            if excluded_dirs.contains(&name) {
                return true;
            }

            if IGNORED_FILES.contains(&name) {
                return true;
            }

            if name.ends_with(".tmp") || name.ends_with(".log") {
                return true;
            }
        }

        false
    }

    pub fn is_binary(&self, path: &Path) -> Result<bool> {
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

pub struct ListFilesTool {
    helpers: ToolHelpers,
}

impl ListFilesTool {
    pub fn new(repo_path: PathBuf, language_registry: Arc<LanguageRegistry>) -> Result<Self> {
        Ok(Self {
            helpers: ToolHelpers::new(repo_path, language_registry)?,
        })
    }
}

#[async_trait]
impl Tool for ListFilesTool {
    fn name(&self) -> &'static str {
        "list_files"
    }

    fn description(&self) -> &'static str {
        "List files in a directory with optional glob pattern filtering and depth control"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Relative path from repository root (e.g., 'src', 'tests'). Use '.' for root."
                },
                "pattern": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., '*.extension', 'filename.*')"
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum directory depth to traverse. Default is 2.",
                    "minimum": 1
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or(".").trim_start_matches('/');
        let pattern = args["pattern"].as_str();
        let max_depth = args["max_depth"].as_u64().map(|d| d as usize);

        debug!(path, pattern, max_depth, "list_files parameters");

        let target_path = self.helpers.validate_path(path)?;

        let mut walker = WalkDir::new(&target_path);
        if let Some(depth) = max_depth {
            walker = walker.max_depth(depth);
        }

        let glob_pattern = pattern.map(Pattern::new).transpose()?;

        let mut results = Vec::new();
        for entry in walker
            .into_iter()
            .filter_entry(|e| !self.helpers.is_ignored(e.path()))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path_obj = entry.path();

            if path_obj.is_file() {
                let rel_path = path_obj
                    .strip_prefix(&target_path)
                    .unwrap_or(path_obj)
                    .to_string_lossy()
                    .to_string();

                if let Some(ref glob) = glob_pattern {
                    if glob.matches(&rel_path) {
                        results.push(rel_path);
                    }
                } else {
                    results.push(rel_path);
                }
            }
        }

        debug!(files_found = results.len(), "list_files completed");

        if results.is_empty() {
            Ok(format!("No files found in {}", path))
        } else {
            Ok(results.join("\n"))
        }
    }
}

pub struct ReadFileTool {
    helpers: ToolHelpers,
}

impl ReadFileTool {
    pub fn new(repo_path: PathBuf, language_registry: Arc<LanguageRegistry>) -> Result<Self> {
        Ok(Self {
            helpers: ToolHelpers::new(repo_path, language_registry)?,
        })
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a specific file with optional line limit"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to repository root (e.g., 'config.ext', 'directory/file.ext')"
                },
                "max_lines": {
                    "type": "integer",
                    "description": "Maximum number of lines to return. Default is 500. Binary files cannot be read.",
                    "minimum": 1
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?
            .trim_start_matches('/');
        let max_lines = args["max_lines"]
            .as_u64()
            .map(|m| m as usize)
            .unwrap_or(DEFAULT_MAX_LINES);

        debug!(path, max_lines, "read_file parameters");

        let file_path = self.helpers.validate_path(path)?;

        if !file_path.is_file() {
            return Err(anyhow!("Not a file: {}", path));
        }

        let metadata = fs::metadata(&file_path).context("Failed to read file metadata")?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(anyhow!(
                "File too large: {} bytes (max: {} bytes)",
                metadata.len(),
                MAX_FILE_SIZE
            ));
        }

        if self.helpers.is_binary(&file_path)? {
            return Err(anyhow!("Cannot read binary file: {}", path));
        }

        let content = fs::read_to_string(&file_path).context("Failed to read file")?;

        let lines: Vec<&str> = content.lines().take(max_lines).collect();
        let total_lines = content.lines().count();

        debug!(
            lines_returned = lines.len(),
            total_lines, "read_file completed"
        );

        let result = lines.join("\n");
        if total_lines > max_lines {
            Ok(format!(
                "{}\n\n[... truncated, showing {} of {} lines]",
                result, max_lines, total_lines
            ))
        } else {
            Ok(result)
        }
    }
}

pub struct SearchFilesTool {
    helpers: ToolHelpers,
}

impl SearchFilesTool {
    pub fn new(repo_path: PathBuf, language_registry: Arc<LanguageRegistry>) -> Result<Self> {
        Ok(Self {
            helpers: ToolHelpers::new(repo_path, language_registry)?,
        })
    }
}

#[async_trait]
impl Tool for SearchFilesTool {
    fn name(&self) -> &'static str {
        "search_files"
    }

    fn description(&self) -> &'static str {
        "Search for files by name pattern across the entire repository"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern for file name matching (e.g., '**/filename.ext', 'buildfile*', '*.extension')"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matching files to return. Default is 20.",
                    "minimum": 1
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'pattern' parameter"))?;
        let max_results = args["max_results"]
            .as_u64()
            .map(|m| m as usize)
            .unwrap_or(DEFAULT_MAX_RESULTS);

        debug!(pattern, max_results, "search_files parameters");

        let glob_pattern = Pattern::new(pattern).context("Invalid glob pattern")?;
        let mut results = Vec::new();

        for entry in WalkDir::new(&self.helpers.repo_path)
            .into_iter()
            .filter_entry(|e| !self.helpers.is_ignored(e.path()))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.is_file() {
                let rel_path = path
                    .strip_prefix(&self.helpers.repo_path)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                if glob_pattern.matches(&rel_path) {
                    results.push(rel_path);
                    if results.len() >= max_results {
                        break;
                    }
                }
            }
        }

        debug!(files_found = results.len(), "search_files completed");

        if results.is_empty() {
            Ok(format!("No files found matching pattern: {}", pattern))
        } else {
            Ok(results.join("\n"))
        }
    }
}

pub struct GetFileTreeTool {
    helpers: ToolHelpers,
}

impl GetFileTreeTool {
    pub fn new(repo_path: PathBuf, language_registry: Arc<LanguageRegistry>) -> Result<Self> {
        Ok(Self {
            helpers: ToolHelpers::new(repo_path, language_registry)?,
        })
    }
}

#[async_trait]
impl Tool for GetFileTreeTool {
    fn name(&self) -> &'static str {
        "get_file_tree"
    }

    fn description(&self) -> &'static str {
        "Get a JSON tree view of the repository structure starting from a path. Returns hierarchical JSON with 'name', 'type' (file/directory), and 'children' fields."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Starting path for the tree. Default is '.' (repository root)."
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum depth of the tree. Default is 2.",
                    "minimum": 1
                }
            },
            "required": []
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"].as_str().unwrap_or(".").trim_start_matches('/');
        let depth = args["depth"]
            .as_u64()
            .map(|d| d as usize)
            .unwrap_or(DEFAULT_TREE_DEPTH);

        debug!(path, depth, "get_file_tree parameters");

        let target_path = self.helpers.validate_path(path)?;
        let tree_json = self.helpers.build_tree_json(&target_path, 0, depth)?;

        serde_json::to_string_pretty(&tree_json).context("Failed to serialize file tree to JSON")
    }
}

pub struct GrepContentTool {
    helpers: ToolHelpers,
}

impl GrepContentTool {
    pub fn new(repo_path: PathBuf, language_registry: Arc<LanguageRegistry>) -> Result<Self> {
        Ok(Self {
            helpers: ToolHelpers::new(repo_path, language_registry)?,
        })
    }
}

#[async_trait]
impl Tool for GrepContentTool {
    fn name(&self) -> &'static str {
        "grep_content"
    }

    fn description(&self) -> &'static str {
        "Search for text patterns within file contents using regex"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for in file contents (e.g., 'function_pattern', 'import.*Module', 'key.*=.*\"value_pattern\"')"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Optional glob pattern to filter which files to search (e.g., '*.ext', 'directory/**/*.extension')"
                },
                "max_matches": {
                    "type": "integer",
                    "description": "Maximum number of matching lines to return. Default is 10.",
                    "minimum": 1
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
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

        for entry in WalkDir::new(&self.helpers.repo_path)
            .into_iter()
            .filter_entry(|e| !self.helpers.is_ignored(e.path()))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let rel_path = path
                .strip_prefix(&self.helpers.repo_path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            if let Some(ref glob) = file_glob {
                if !glob.matches(&rel_path) {
                    continue;
                }
            }

            if self.helpers.is_binary(path).unwrap_or(true) {
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
}

pub struct GetBestPracticesTool {
    language_registry: Arc<LanguageRegistry>,
}

impl GetBestPracticesTool {
    pub fn new(language_registry: Arc<LanguageRegistry>) -> Self {
        Self { language_registry }
    }
}

#[async_trait]
impl Tool for GetBestPracticesTool {
    fn name(&self) -> &'static str {
        "get_best_practices"
    }

    fn description(&self) -> &'static str {
        "Get recommended best practices template for a specific language and build system combination"
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "language": {
                    "type": "string",
                    "description": "Programming language (e.g., 'Rust', 'JavaScript', 'Java', 'Python', 'Go', 'C++', '.NET', 'Ruby')"
                },
                "build_system": {
                    "type": "string",
                    "description": "Build system/package manager (e.g., 'cargo', 'npm', 'yarn', 'pnpm', 'bun', 'maven', 'gradle', 'pip', 'poetry', 'pipenv', 'go', 'cmake', 'make', 'dotnet', 'bundler')"
                }
            },
            "required": ["language", "build_system"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let language = args["language"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'language' parameter"))?;
        let build_system = args["build_system"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'build_system' parameter"))?;

        debug!(language, build_system, "get_best_practices parameters");

        let lang_def = self
            .language_registry
            .get_language(language)
            .ok_or_else(|| anyhow!("Language '{}' not found in registry", language))?;

        let template = lang_def.build_template(build_system).ok_or_else(|| {
            anyhow!(
                "No template found for language '{}' with build system '{}'",
                language,
                build_system
            )
        })?;

        info!(
            language,
            build_system, "Best practices template retrieved successfully"
        );

        serde_json::to_string_pretty(&template)
            .context("Failed to serialize best practices template")
    }
}

pub struct SubmitDetectionTool;

#[async_trait]
impl Tool for SubmitDetectionTool {
    fn name(&self) -> &'static str {
        "submit_detection"
    }

    fn description(&self) -> &'static str {
        "Submit the final UniversalBuild specification"
    }

    fn schema(&self) -> Value {
        // The schema is quite large, so we'll reference the existing registry implementation
        json!({
            "type": "object",
            "properties": {
                "version": { "type": "string", "enum": ["1.0"] },
                "metadata": { "type": "object" },
                "build": { "type": "object" },
                "runtime": { "type": "object" }
            },
            "required": ["version", "metadata", "build", "runtime"]
        })
    }

    async fn execute(&self, args: Value) -> Result<String> {
        info!("LLM submitting final UniversalBuild detection result");
        debug!(universal_build = ?args, "UniversalBuild submission");

        let universal_build: UniversalBuild = serde_json::from_value(args)
            .context("Failed to parse UniversalBuild from LLM response")?;

        crate::validation::Validator::new()
            .validate(&universal_build)
            .context("UniversalBuild validation failed")?;

        info!(
            language = %universal_build.metadata.language,
            build_system = %universal_build.metadata.build_system,
            confidence = %universal_build.metadata.confidence,
            "UniversalBuild validated successfully"
        );

        serde_json::to_string_pretty(&universal_build).context("Failed to serialize UniversalBuild")
    }
}
