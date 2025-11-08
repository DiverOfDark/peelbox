//! Tool registry for LLM-based detection
//!
//! This module provides the ToolRegistry which creates genai Tool definitions
//! with JSON schemas for all available repository analysis tools.

use genai::chat::Tool;
use serde_json::json;

use super::definitions::*;

pub struct ToolRegistry;

impl ToolRegistry {
    /// Create all available tools for repository analysis
    pub fn create_all_tools() -> Vec<Tool> {
        vec![
            Self::create_list_files_tool(),
            Self::create_read_file_tool(),
            Self::create_search_files_tool(),
            Self::create_get_file_tree_tool(),
            Self::create_grep_content_tool(),
            Self::create_submit_detection_tool(),
        ]
    }

    fn create_list_files_tool() -> Tool {
        Tool {
            name: TOOL_LIST_FILES.to_string(),
            description: Some(
                "List files in a directory with optional glob pattern filtering and depth control"
                    .to_string(),
            ),
            schema: Some(json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path from repository root (e.g., 'src', 'tests'). Use '.' for root."
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Optional glob pattern to filter files (e.g., '*.rs', '*.toml', 'Cargo.*')"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum directory depth to traverse. Default is 2.",
                        "minimum": 1
                    }
                },
                "required": ["path"]
            })),
            config: None,
        }
    }

    fn create_read_file_tool() -> Tool {
        Tool {
            name: TOOL_READ_FILE.to_string(),
            description: Some(
                "Read the contents of a specific file with optional line limit".to_string(),
            ),
            schema: Some(json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "File path relative to repository root (e.g., 'Cargo.toml', 'src/main.rs')"
                    },
                    "max_lines": {
                        "type": "integer",
                        "description": "Maximum number of lines to return. Default is 500. Binary files cannot be read.",
                        "minimum": 1
                    }
                },
                "required": ["path"]
            })),
            config: None,
        }
    }

    fn create_search_files_tool() -> Tool {
        Tool {
            name: TOOL_SEARCH_FILES.to_string(),
            description: Some(
                "Search for files by name pattern across the entire repository".to_string(),
            ),
            schema: Some(json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern for file name matching (e.g., '**/package.json', 'Dockerfile*', '*.gradle')"
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of matching files to return. Default is 20.",
                        "minimum": 1
                    }
                },
                "required": ["pattern"]
            })),
            config: None,
        }
    }

    fn create_get_file_tree_tool() -> Tool {
        Tool {
            name: TOOL_GET_FILE_TREE.to_string(),
            description: Some(
                "Get a tree view of the repository structure starting from a path".to_string(),
            ),
            schema: Some(json!({
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
            })),
            config: None,
        }
    }

    fn create_grep_content_tool() -> Tool {
        Tool {
            name: TOOL_GREP_CONTENT.to_string(),
            description: Some(
                "Search for text patterns within file contents using regex".to_string(),
            ),
            schema: Some(json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for in file contents (e.g., 'fn main', 'import.*React', 'version.*=.*\"\\d+\\.\\d+\"')"
                    },
                    "file_pattern": {
                        "type": "string",
                        "description": "Optional glob pattern to filter which files to search (e.g., '*.rs', '*.js', 'src/**/*.py')"
                    },
                    "max_matches": {
                        "type": "integer",
                        "description": "Maximum number of matching lines to return. Default is 10.",
                        "minimum": 1
                    }
                },
                "required": ["pattern"]
            })),
            config: None,
        }
    }

    fn create_submit_detection_tool() -> Tool {
        Tool {
            name: TOOL_SUBMIT_DETECTION.to_string(),
            description: Some(
                "Submit the final build system detection result. Call this once you have gathered enough information about the repository.".to_string(),
            ),
            schema: Some(json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": "Primary programming language (e.g., 'Rust', 'JavaScript', 'Python', 'Go')"
                    },
                    "build_system": {
                        "type": "string",
                        "description": "Detected build system (e.g., 'cargo', 'npm', 'gradle', 'maven', 'make', 'go')"
                    },
                    "build_command": {
                        "type": "string",
                        "description": "Command to build the project (e.g., 'cargo build', 'npm run build', 'make')"
                    },
                    "test_command": {
                        "type": "string",
                        "description": "Command to run tests (e.g., 'cargo test', 'npm test', 'pytest'). Optional."
                    },
                    "runtime": {
                        "type": "string",
                        "description": "Runtime environment (e.g., 'native', 'node', 'jvm', 'python', 'docker')"
                    },
                    "dependencies": {
                        "type": "array",
                        "description": "List of key dependencies or packages detected",
                        "items": {
                            "type": "string"
                        }
                    },
                    "entry_point": {
                        "type": "string",
                        "description": "Main entry point file or command (e.g., 'src/main.rs', 'index.js', 'main.py')"
                    },
                    "dev_command": {
                        "type": "string",
                        "description": "Command to run in development mode (e.g., 'cargo run', 'npm run dev'). Optional."
                    },
                    "confidence": {
                        "type": "number",
                        "description": "Confidence score from 0.0 to 1.0 indicating how certain the detection is",
                        "minimum": 0.0,
                        "maximum": 1.0
                    },
                    "reasoning": {
                        "type": "string",
                        "description": "Brief explanation of how the build system was detected and key evidence found"
                    },
                    "warnings": {
                        "type": "array",
                        "description": "Any warnings or potential issues detected (e.g., missing files, unusual configurations)",
                        "items": {
                            "type": "string"
                        }
                    }
                },
                "required": [
                    "language",
                    "build_system",
                    "build_command",
                    "runtime",
                    "dependencies",
                    "entry_point",
                    "confidence",
                    "reasoning",
                    "warnings"
                ]
            })),
            config: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_count() {
        let tools = ToolRegistry::create_all_tools();
        assert_eq!(tools.len(), 6, "Expected 6 tools to be registered");
    }

    #[test]
    fn test_tool_names() {
        let tools = ToolRegistry::create_all_tools();
        let tool_names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();

        assert!(tool_names.contains(&TOOL_LIST_FILES.to_string()));
        assert!(tool_names.contains(&TOOL_READ_FILE.to_string()));
        assert!(tool_names.contains(&TOOL_SEARCH_FILES.to_string()));
        assert!(tool_names.contains(&TOOL_GET_FILE_TREE.to_string()));
        assert!(tool_names.contains(&TOOL_GREP_CONTENT.to_string()));
        assert!(tool_names.contains(&TOOL_SUBMIT_DETECTION.to_string()));
    }

    #[test]
    fn test_all_tools_have_descriptions() {
        let tools = ToolRegistry::create_all_tools();
        for tool in tools {
            assert!(
                tool.description.is_some(),
                "Tool {} missing description",
                tool.name
            );
            assert!(
                !tool.description.unwrap().is_empty(),
                "Tool {} has empty description",
                tool.name
            );
        }
    }

    #[test]
    fn test_all_tools_have_schemas() {
        let tools = ToolRegistry::create_all_tools();
        for tool in tools {
            assert!(
                tool.schema.is_some(),
                "Tool {} missing schema",
                tool.name
            );
        }
    }

    #[test]
    fn test_list_files_schema() {
        let tool = ToolRegistry::create_list_files_tool();
        let schema = tool.schema.unwrap();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["pattern"].is_object());
        assert!(schema["properties"]["max_depth"].is_object());
        assert_eq!(schema["required"], json!(["path"]));
    }

    #[test]
    fn test_read_file_schema() {
        let tool = ToolRegistry::create_read_file_tool();
        let schema = tool.schema.unwrap();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["max_lines"].is_object());
        assert_eq!(schema["required"], json!(["path"]));
    }

    #[test]
    fn test_search_files_schema() {
        let tool = ToolRegistry::create_search_files_tool();
        let schema = tool.schema.unwrap();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
        assert!(schema["properties"]["max_results"].is_object());
        assert_eq!(schema["required"], json!(["pattern"]));
    }

    #[test]
    fn test_get_file_tree_schema() {
        let tool = ToolRegistry::create_get_file_tree_tool();
        let schema = tool.schema.unwrap();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["path"].is_object());
        assert!(schema["properties"]["depth"].is_object());
        assert_eq!(schema["required"], json!([]));
    }

    #[test]
    fn test_grep_content_schema() {
        let tool = ToolRegistry::create_grep_content_tool();
        let schema = tool.schema.unwrap();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["pattern"].is_object());
        assert!(schema["properties"]["file_pattern"].is_object());
        assert!(schema["properties"]["max_matches"].is_object());
        assert_eq!(schema["required"], json!(["pattern"]));
    }

    #[test]
    fn test_submit_detection_schema() {
        let tool = ToolRegistry::create_submit_detection_tool();
        let schema = tool.schema.unwrap();

        assert_eq!(schema["type"], "object");

        // Verify all required fields are present in schema
        let required_fields = [
            "language",
            "build_system",
            "build_command",
            "runtime",
            "dependencies",
            "entry_point",
            "confidence",
            "reasoning",
            "warnings",
        ];

        for field in &required_fields {
            assert!(
                schema["properties"][field].is_object(),
                "Missing property: {}",
                field
            );
        }

        // Verify optional fields
        assert!(schema["properties"]["test_command"].is_object());
        assert!(schema["properties"]["dev_command"].is_object());

        // Verify required array
        let required_array = schema["required"].as_array().unwrap();
        assert_eq!(required_array.len(), 9);
    }

    #[test]
    fn test_schema_types() {
        let tool = ToolRegistry::create_submit_detection_tool();
        let schema = tool.schema.unwrap();

        assert_eq!(schema["properties"]["language"]["type"], "string");
        assert_eq!(schema["properties"]["confidence"]["type"], "number");
        assert_eq!(schema["properties"]["dependencies"]["type"], "array");
        assert_eq!(schema["properties"]["warnings"]["type"], "array");
    }

    #[test]
    fn test_confidence_constraints() {
        let tool = ToolRegistry::create_submit_detection_tool();
        let schema = tool.schema.unwrap();

        let confidence_schema = &schema["properties"]["confidence"];
        assert_eq!(confidence_schema["minimum"], 0.0);
        assert_eq!(confidence_schema["maximum"], 1.0);
    }
}
