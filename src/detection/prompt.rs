//! Prompt engineering for build system detection
//!
//! This module handles the construction of sophisticated prompts that guide
//! the LLM to accurately detect build systems and generate appropriate commands.
//! The prompts are carefully designed to:
//!
//! - Provide clear context from the repository
//! - Request structured JSON responses
//! - Guide the LLM towards high-quality detections
//! - Handle edge cases and warnings
//!
//! # Example
//!
//! ```
//! use aipack::detection::prompt::PromptBuilder;
//! use aipack::detection::types::RepositoryContext;
//! use std::path::PathBuf;
//!
//! let context = RepositoryContext::minimal(
//!     PathBuf::from("/path/to/repo"),
//!     "repo/\n├── Cargo.toml\n└── src/".to_string(),
//! );
//!
//! let prompt = PromptBuilder::build_detection_prompt(&context);
//! assert!(prompt.contains("REPOSITORY INFORMATION"));
//! ```

use crate::detection::types::RepositoryContext;
use std::collections::HashMap;

/// Maximum number of characters to include from file contents
const MAX_FILE_CONTENT_CHARS: usize = 8000;

/// Maximum number of characters for the file tree
const MAX_FILE_TREE_CHARS: usize = 4000;

/// Maximum number of characters for README content
const MAX_README_CHARS: usize = 4000;

/// Prompt builder for LLM-based detection
///
/// This struct provides methods for constructing prompts that guide the LLM
/// to detect build systems accurately. The prompts are engineered to:
///
/// - Maximize signal-to-noise ratio
/// - Request JSON-formatted responses
/// - Include relevant context without overwhelming the model
/// - Guide towards actionable, correct commands
pub struct PromptBuilder;

impl PromptBuilder {
    /// Builds a detection prompt from repository context
    ///
    /// This is the main method that constructs a comprehensive prompt for
    /// the LLM, including:
    ///
    /// - System instructions
    /// - Repository structure (file tree)
    /// - Configuration file contents
    /// - README content (if available)
    /// - Response format specification
    ///
    /// # Arguments
    ///
    /// * `context` - Repository context containing all relevant information
    ///
    /// # Returns
    ///
    /// A formatted prompt string ready to send to the LLM
    ///
    /// # Example
    ///
    /// ```
    /// use aipack::detection::prompt::PromptBuilder;
    /// use aipack::detection::types::RepositoryContext;
    /// use std::path::PathBuf;
    ///
    /// let context = RepositoryContext::minimal(
    ///     PathBuf::from("/test/repo"),
    ///     "test/\n└── Cargo.toml".to_string(),
    /// ).with_key_file(
    ///     "Cargo.toml".to_string(),
    ///     "[package]\nname = \"test\"".to_string(),
    /// );
    ///
    /// let prompt = PromptBuilder::build_detection_prompt(&context);
    /// assert!(prompt.contains("Cargo.toml"));
    /// ```
    pub fn build_detection_prompt(context: &RepositoryContext) -> String {
        let repo_path = context.repo_path.display().to_string();
        let file_tree = Self::format_file_tree(&context.file_tree);
        let key_files = Self::format_key_files(&context.key_files);
        let readme = Self::format_readme(&context.readme_content);

        format!(
            r#"You are an expert software engineer specializing in containerization and Docker image building.

Analyze the following repository information and determine what's needed to build an optimized Docker image for this project.

REPOSITORY INFORMATION:
Path: {repo_path}

FILE STRUCTURE:
{file_tree}

KEY CONFIGURATION FILES:
{key_files}

README:
{readme}

TASK:
Based on the above repository information, identify:
1. The primary programming language
2. The build system/package manager being used
3. The command to build/compile the project (for Docker RUN)
4. The command to run tests (for Docker RUN during build)
5. The Docker runtime/base image to use (e.g., 'python:3.11', 'node:20', 'rust:latest')
6. System dependencies/packages needed in the Docker image (e.g., ['curl', 'ca-certificates'])
7. The entry point command - what runs when the container starts (e.g., 'java -jar app.jar')
8. Your confidence level (0.0-1.0) that this configuration is correct
9. Brief explanation of why you chose these options
10. Any potential issues or warnings

RESPONSE FORMAT:
You MUST respond with valid JSON only (no markdown, no code blocks).
No preamble, no explanation before the JSON.

{{
  "language": "string (e.g., 'Rust', 'JavaScript', 'Python')",
  "build_system": "string (e.g., 'cargo', 'npm', 'gradle')",
  "build_command": "string (complete command to build the application)",
  "test_command": "string (complete command to run tests)",
  "runtime": "string (Docker runtime, e.g., 'python:3.11-slim', 'node:20', 'rust:1.75')",
  "dependencies": ["list of system packages needed (e.g., 'curl', 'openssl')"],
  "entry_point": "string (command to start the application in container, e.g., 'java -jar app.jar')",
  "dev_command": "string or null (optional: command for development/watch mode)",
  "confidence": 0.85,
  "reasoning": "string (1-2 sentences explaining the Docker configuration)",
  "warnings": ["list of", "potential issues"]
}}

IMPORTANT:
- Return ONLY the JSON object, nothing else
- Do not wrap the JSON in markdown code blocks
- Ensure all JSON is properly escaped
- Focus on minimal dependencies suitable for distroless/minimal base images (security first)
- Runtime should be a specific base image string that can be used in FROM instruction
- Entry point should be the exact command to start the application
- Confidence should reflect certainty (0.9+ = very confident, 0.7-0.9 = confident, 0.5-0.7 = moderate, <0.5 = uncertain)
- If multiple build systems are detected, choose the primary one
- Include optimization flags in build commands (e.g., --release for production builds)
- Include only essential dependencies - prefer minimal/distroless images where possible
"#,
            repo_path = repo_path,
            file_tree = file_tree,
            key_files = key_files,
            readme = readme
        )
    }

    /// Formats key configuration files for inclusion in the prompt
    ///
    /// This method takes a map of file paths to contents and formats them
    /// in a readable way, with truncation to avoid overwhelming the model.
    ///
    /// # Arguments
    ///
    /// * `key_files` - Map of file paths to their contents
    ///
    /// # Returns
    ///
    /// Formatted string representing all key files
    fn format_key_files(key_files: &HashMap<String, String>) -> String {
        if key_files.is_empty() {
            return "(No configuration files detected)".to_string();
        }

        let mut formatted = String::new();

        // Sort files for consistent ordering
        let mut sorted_files: Vec<_> = key_files.iter().collect();
        sorted_files.sort_by_key(|(path, _)| *path);

        for (path, content) in sorted_files {
            formatted.push_str(&format!("\n--- {} ---\n", path));

            // Truncate long files
            let truncated = Self::truncate_text(content, MAX_FILE_CONTENT_CHARS);
            formatted.push_str(&truncated);

            if truncated.len() < content.len() {
                formatted.push_str(&format!(
                    "\n... (truncated {} chars) ...\n",
                    content.len() - truncated.len()
                ));
            }

            formatted.push('\n');
        }

        formatted
    }

    /// Formats the file tree for inclusion in the prompt
    ///
    /// Ensures the file tree is not too long by truncating if necessary.
    ///
    /// # Arguments
    ///
    /// * `tree` - File tree string (typically from `tree` command or similar)
    ///
    /// # Returns
    ///
    /// Formatted and possibly truncated file tree
    fn format_file_tree(tree: &str) -> String {
        let truncated = Self::truncate_text(tree, MAX_FILE_TREE_CHARS);

        if truncated.len() < tree.len() {
            format!(
                "{}\n... (truncated {} chars) ...",
                truncated,
                tree.len() - truncated.len()
            )
        } else {
            truncated
        }
    }

    /// Formats README content for inclusion in the prompt
    ///
    /// # Arguments
    ///
    /// * `readme` - Optional README content
    ///
    /// # Returns
    ///
    /// Formatted README or a placeholder message
    fn format_readme(readme: &Option<String>) -> String {
        match readme {
            Some(content) if !content.trim().is_empty() => {
                let truncated = Self::truncate_text(content, MAX_README_CHARS);

                if truncated.len() < content.len() {
                    format!(
                        "{}\n... (truncated {} chars) ...",
                        truncated,
                        content.len() - truncated.len()
                    )
                } else {
                    truncated
                }
            }
            _ => "(No README found)".to_string(),
        }
    }

    /// Truncates text to a maximum number of characters
    ///
    /// Tries to truncate at line boundaries when possible for cleaner output.
    ///
    /// # Arguments
    ///
    /// * `text` - Text to truncate
    /// * `max_chars` - Maximum number of characters
    ///
    /// # Returns
    ///
    /// Truncated text (or original if shorter than max)
    fn truncate_text(text: &str, max_chars: usize) -> String {
        if text.len() <= max_chars {
            return text.to_string();
        }

        // Try to truncate at a line boundary
        let truncated = &text[..max_chars];

        if let Some(last_newline) = truncated.rfind('\n') {
            // Truncate at the last newline before max_chars
            truncated[..last_newline].to_string()
        } else {
            // No newline found, just truncate at max_chars
            truncated.to_string()
        }
    }

    /// Escapes a string for safe inclusion in JSON
    ///
    /// This is primarily used internally but exposed for testing purposes.
    ///
    /// # Arguments
    ///
    /// * `s` - String to escape
    ///
    /// # Returns
    ///
    /// JSON-safe escaped string
    pub fn escape_json_string(s: &str) -> String {
        s.chars()
            .flat_map(|c| match c {
                '"' => vec!['\\', '"'],
                '\\' => vec!['\\', '\\'],
                '\n' => vec!['\\', 'n'],
                '\r' => vec!['\\', 'r'],
                '\t' => vec!['\\', 't'],
                c if c.is_control() => format!("\\u{:04x}", c as u32).chars().collect(),
                c => vec![c],
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_detection_prompt_minimal() {
        let context = RepositoryContext::minimal(
            PathBuf::from("/test/repo"),
            "test/\n└── file.txt".to_string(),
        );

        let prompt = PromptBuilder::build_detection_prompt(&context);

        assert!(prompt.contains("REPOSITORY INFORMATION"));
        assert!(prompt.contains("FILE STRUCTURE"));
        assert!(prompt.contains("KEY CONFIGURATION FILES"));
        assert!(prompt.contains("README"));
        assert!(prompt.contains("TASK"));
        assert!(prompt.contains("RESPONSE FORMAT"));
        assert!(prompt.contains("/test/repo"));
        assert!(prompt.contains("test/"));
    }

    #[test]
    fn test_build_detection_prompt_with_files() {
        let context = RepositoryContext::minimal(
            PathBuf::from("/test/repo"),
            "test/\n├── Cargo.toml\n└── src/".to_string(),
        )
        .with_key_file(
            "Cargo.toml".to_string(),
            "[package]\nname = \"test\"".to_string(),
        )
        .with_readme("# Test Project\n\nBuild with cargo.".to_string());

        let prompt = PromptBuilder::build_detection_prompt(&context);

        assert!(prompt.contains("Cargo.toml"));
        assert!(prompt.contains("[package]"));
        assert!(prompt.contains("Test Project"));
        assert!(prompt.contains("Build with cargo"));
    }

    #[test]
    fn test_format_key_files_empty() {
        let files = HashMap::new();
        let formatted = PromptBuilder::format_key_files(&files);
        assert_eq!(formatted, "(No configuration files detected)");
    }

    #[test]
    fn test_format_key_files_single() {
        let mut files = HashMap::new();
        files.insert(
            "package.json".to_string(),
            r#"{"name": "test", "version": "1.0.0"}"#.to_string(),
        );

        let formatted = PromptBuilder::format_key_files(&files);
        assert!(formatted.contains("--- package.json ---"));
        assert!(formatted.contains("\"name\": \"test\""));
    }

    #[test]
    fn test_format_key_files_multiple() {
        let mut files = HashMap::new();
        files.insert("Cargo.toml".to_string(), "[package]".to_string());
        files.insert("README.md".to_string(), "# Test".to_string());

        let formatted = PromptBuilder::format_key_files(&files);

        // Should be sorted alphabetically
        let cargo_pos = formatted.find("Cargo.toml").unwrap();
        let readme_pos = formatted.find("README.md").unwrap();
        assert!(cargo_pos < readme_pos);
    }

    #[test]
    fn test_format_key_files_truncation() {
        let mut files = HashMap::new();
        let long_content = "x".repeat(MAX_FILE_CONTENT_CHARS + 1000);
        files.insert("large.txt".to_string(), long_content.clone());

        let formatted = PromptBuilder::format_key_files(&files);
        assert!(formatted.contains("--- large.txt ---"));
        assert!(formatted.contains("truncated"));
        assert!(formatted.len() < long_content.len());
    }

    #[test]
    fn test_format_file_tree_normal() {
        let tree = "project/\n├── src/\n│   └── main.rs\n└── Cargo.toml";
        let formatted = PromptBuilder::format_file_tree(tree);
        assert_eq!(formatted, tree);
    }

    #[test]
    fn test_format_file_tree_truncation() {
        let long_tree = "line\n".repeat(MAX_FILE_TREE_CHARS);
        let formatted = PromptBuilder::format_file_tree(&long_tree);
        assert!(formatted.contains("truncated"));
        assert!(formatted.len() < long_tree.len());
    }

    #[test]
    fn test_format_readme_none() {
        let formatted = PromptBuilder::format_readme(&None);
        assert_eq!(formatted, "(No README found)");
    }

    #[test]
    fn test_format_readme_empty() {
        let formatted = PromptBuilder::format_readme(&Some("   ".to_string()));
        assert_eq!(formatted, "(No README found)");
    }

    #[test]
    fn test_format_readme_normal() {
        let readme = "# My Project\n\nThis is a test project.";
        let formatted = PromptBuilder::format_readme(&Some(readme.to_string()));
        assert_eq!(formatted, readme);
    }

    #[test]
    fn test_format_readme_truncation() {
        let long_readme = "word ".repeat(MAX_README_CHARS);
        let formatted = PromptBuilder::format_readme(&Some(long_readme.clone()));
        assert!(formatted.contains("truncated"));
        assert!(formatted.len() < long_readme.len());
    }

    #[test]
    fn test_truncate_text_short() {
        let text = "short text";
        let truncated = PromptBuilder::truncate_text(text, 100);
        assert_eq!(truncated, text);
    }

    #[test]
    fn test_truncate_text_at_newline() {
        let text = "line 1\nline 2\nline 3\nline 4";
        let truncated = PromptBuilder::truncate_text(text, 15);
        // Should truncate at "line 1\nline 2\n" (14 chars)
        assert!(truncated.len() <= 15);
        assert!(truncated.ends_with("line 2"));
    }

    #[test]
    fn test_truncate_text_no_newline() {
        let text = "abcdefghijklmnop";
        let truncated = PromptBuilder::truncate_text(text, 10);
        assert_eq!(truncated, "abcdefghij");
    }

    #[test]
    fn test_escape_json_string_basic() {
        assert_eq!(PromptBuilder::escape_json_string("hello"), "hello");
    }

    #[test]
    fn test_escape_json_string_quotes() {
        assert_eq!(
            PromptBuilder::escape_json_string(r#"say "hello""#),
            r#"say \"hello\""#
        );
    }

    #[test]
    fn test_escape_json_string_backslash() {
        assert_eq!(
            PromptBuilder::escape_json_string(r"path\to\file"),
            r"path\\to\\file"
        );
    }

    #[test]
    fn test_escape_json_string_newlines() {
        assert_eq!(
            PromptBuilder::escape_json_string("line1\nline2\r\nline3"),
            r"line1\nline2\r\nline3"
        );
    }

    #[test]
    fn test_escape_json_string_tabs() {
        assert_eq!(
            PromptBuilder::escape_json_string("col1\tcol2"),
            r"col1\tcol2"
        );
    }

    #[test]
    fn test_escape_json_string_control_chars() {
        let input = "text\x01\x1fmore";
        let output = PromptBuilder::escape_json_string(input);
        assert!(output.contains(r"\u0001"));
        assert!(output.contains(r"\u001f"));
    }

    #[test]
    fn test_prompt_requests_json_only() {
        let context = RepositoryContext::minimal(PathBuf::from("/test"), "test/".to_string());

        let prompt = PromptBuilder::build_detection_prompt(&context);

        assert!(prompt.contains("valid JSON only"));
        assert!(prompt.contains("no markdown"));
        assert!(prompt.contains("no code blocks"));
        assert!(prompt.contains("No preamble"));
    }

    #[test]
    fn test_prompt_includes_all_required_fields() {
        let context = RepositoryContext::minimal(PathBuf::from("/test"), "test/".to_string());

        let prompt = PromptBuilder::build_detection_prompt(&context);

        // Check that the JSON schema includes all required Docker fields
        assert!(prompt.contains("\"language\""));
        assert!(prompt.contains("\"build_system\""));
        assert!(prompt.contains("\"build_command\""));
        assert!(prompt.contains("\"test_command\""));
        assert!(prompt.contains("\"runtime\""));  // Docker runtime
        assert!(prompt.contains("\"dependencies\""));  // System packages
        assert!(prompt.contains("\"entry_point\""));  // Container entry point
        assert!(prompt.contains("\"dev_command\""));
        assert!(prompt.contains("\"confidence\""));
        assert!(prompt.contains("\"reasoning\""));
        assert!(prompt.contains("\"warnings\""));
    }
}
