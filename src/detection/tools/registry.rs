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
                        "description": "Optional glob pattern to filter files (e.g., '*.extension', 'filename.*')"
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
                        "description": "File path relative to repository root (e.g., 'config.ext', 'directory/file.ext')"
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
                        "description": "Glob pattern for file name matching (e.g., '**/filename.ext', 'buildfile*', '*.extension')"
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
                "Get a JSON tree view of the repository structure starting from a path. Returns hierarchical JSON with 'name', 'type' (file/directory), and 'children' fields.".to_string(),
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
            })),
            config: None,
        }
    }

    fn create_submit_detection_tool() -> Tool {
        Tool {
            name: TOOL_SUBMIT_DETECTION.to_string(),
            description: Some(
                r#"Submit the final UniversalBuild specification. This is a declarative container build format that describes how to build and run the application.

IMPORTANT: UniversalBuild uses a multi-stage container build approach:
1. Build Stage: Compiles/builds the application with all necessary tools
2. Runtime Stage: Runs the application in a minimal container with only runtime dependencies

Common Language Examples:

Rust (Cargo):
- Build base: "rust:1.75" or "rust:1.75-slim"
- Build commands: ["cargo build --release"]
- Build artifacts: ["target/release/myapp"]
- Runtime base: "debian:bookworm-slim" or "alpine:3.19"
- Runtime copy: [{"from": "target/release/myapp", "to": "/usr/local/bin/myapp"}]
- Runtime command: ["/usr/local/bin/myapp"]

Node.js (npm/yarn/pnpm):
- Build base: "node:20" or "node:20-alpine"
- Build commands: ["npm install", "npm run build"] or ["yarn install", "yarn build"]
- Build artifacts: ["dist", "node_modules"] or ["build"]
- Runtime base: "node:20-slim" or "node:20-alpine"
- Runtime copy: [{"from": "dist", "to": "/app/dist"}, {"from": "node_modules", "to": "/app/node_modules"}]
- Runtime command: ["node", "dist/index.js"]

Python (pip/poetry):
- Build base: "python:3.11" or "python:3.11-slim"
- Build commands: ["pip install -r requirements.txt"] or ["poetry install --no-dev"]
- Build artifacts: ["/usr/local/lib/python3.11/site-packages", "app"]
- Runtime base: "python:3.11-slim" or "python:3.11-alpine"
- Runtime copy: [{"from": "/usr/local/lib/python3.11/site-packages", "to": "/usr/local/lib/python3.11/site-packages"}]
- Runtime command: ["python", "app/main.py"]

Java (Maven/Gradle):
- Build base: "maven:3.9-eclipse-temurin-21" or "gradle:8.5-jdk21"
- Build commands: ["mvn package -DskipTests"] or ["gradle build"]
- Build artifacts: ["target/myapp.jar"] or ["build/libs/myapp.jar"]
- Runtime base: "eclipse-temurin:21-jre-alpine"
- Runtime copy: [{"from": "target/myapp.jar", "to": "/app/app.jar"}]
- Runtime command: ["java", "-jar", "/app/app.jar"]

Go:
- Build base: "golang:1.21" or "golang:1.21-alpine"
- Build commands: ["go build -o myapp ."]
- Build artifacts: ["myapp"]
- Runtime base: "alpine:3.19" or "scratch"
- Runtime copy: [{"from": "myapp", "to": "/myapp"}]
- Runtime command: ["/myapp"]

Key Guidelines:
- version: Always use "1.0"
- confidence: 0.0-1.0 (0.9+ for strong evidence, 0.7-0.9 for good evidence, <0.7 for uncertain)
- build.base: Full image with build tools (compilers, build systems)
- build.context: Source files to copy - pairs of [source, destination], e.g., [".", "/app", "src", "/build/src"]
- build.cache: Directories to cache between builds (e.g., ["/root/.cargo/registry", "/usr/local/cargo/git"])
- build.artifacts: Output files from build stage to preserve
- runtime.base: Minimal image for running the application
- runtime.copy: Copy artifacts from build stage, using {"from": "...", "to": "..."} objects
- runtime.command: Array of strings for entrypoint (e.g., ["./myapp"] or ["python", "main.py"])
- runtime.ports: Expose ports if this is a service (e.g., [8080, 8443])
- runtime.healthcheck: Optional health check for services

Call this tool once you have analyzed the repository and determined the build approach."#.to_string(),
            ),
            schema: Some(json!({
                "type": "object",
                "properties": {
                    "version": {
                        "type": "string",
                        "description": "Schema version (always '1.0')",
                        "enum": ["1.0"]
                    },
                    "metadata": {
                        "type": "object",
                        "description": "Project metadata and detection information",
                        "properties": {
                            "project_name": {
                                "type": "string",
                                "description": "Optional project name (if detected from config files)"
                            },
                            "language": {
                                "type": "string",
                                "description": "Primary programming language (e.g., 'rust', 'nodejs', 'python', 'java', 'go')"
                            },
                            "build_system": {
                                "type": "string",
                                "description": "Build system name (e.g., 'cargo', 'npm', 'maven', 'gradle', 'go')"
                            },
                            "confidence": {
                                "type": "number",
                                "description": "Confidence score from 0.0 to 1.0",
                                "minimum": 0.0,
                                "maximum": 1.0
                            },
                            "reasoning": {
                                "type": "string",
                                "description": "Explanation of detection reasoning and key evidence"
                            }
                        },
                        "required": ["language", "build_system", "confidence", "reasoning"]
                    },
                    "build": {
                        "type": "object",
                        "description": "Build stage configuration - defines how to compile/build the application",
                        "properties": {
                            "base": {
                                "type": "string",
                                "description": "Base Docker image for build stage with build tools (e.g., 'rust:1.75', 'node:20', 'python:3.11')"
                            },
                            "packages": {
                                "type": "array",
                                "description": "System packages to install (e.g., ['build-essential', 'pkg-config', 'libssl-dev'])",
                                "items": {
                                    "type": "string"
                                },
                                "default": []
                            },
                            "env": {
                                "type": "object",
                                "description": "Environment variables for build stage (e.g., {'CARGO_INCREMENTAL': '0'})",
                                "additionalProperties": {
                                    "type": "string"
                                },
                                "default": {}
                            },
                            "commands": {
                                "type": "array",
                                "description": "Build commands to execute in order (e.g., ['cargo build --release'])",
                                "items": {
                                    "type": "string"
                                },
                                "minItems": 1
                            },
                            "context": {
                                "type": "array",
                                "description": "Files/directories to copy from source as pairs [source, destination] (e.g., ['.', '/app'] means copy current dir to /app)",
                                "items": {
                                    "type": "string"
                                },
                                "minItems": 2
                            },
                            "cache": {
                                "type": "array",
                                "description": "Directories to cache between builds (e.g., ['/root/.cargo/registry', '/usr/local/cargo/git'])",
                                "items": {
                                    "type": "string"
                                },
                                "default": []
                            },
                            "artifacts": {
                                "type": "array",
                                "description": "Build artifacts to preserve (e.g., ['target/release/myapp'])",
                                "items": {
                                    "type": "string"
                                },
                                "minItems": 1
                            }
                        },
                        "required": ["base", "commands", "context", "artifacts"]
                    },
                    "runtime": {
                        "type": "object",
                        "description": "Runtime stage configuration - defines the final container environment",
                        "properties": {
                            "base": {
                                "type": "string",
                                "description": "Base Docker image for runtime (minimal, e.g., 'debian:bookworm-slim', 'alpine:3.19', 'scratch')"
                            },
                            "packages": {
                                "type": "array",
                                "description": "Runtime system packages (e.g., ['ca-certificates', 'libssl3'])",
                                "items": {
                                    "type": "string"
                                },
                                "default": []
                            },
                            "env": {
                                "type": "object",
                                "description": "Runtime environment variables (e.g., {'PORT': '8080', 'NODE_ENV': 'production'})",
                                "additionalProperties": {
                                    "type": "string"
                                },
                                "default": {}
                            },
                            "copy": {
                                "type": "array",
                                "description": "Files to copy from build stage as objects with 'from' and 'to' fields",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "from": {
                                            "type": "string",
                                            "description": "Source path in build stage"
                                        },
                                        "to": {
                                            "type": "string",
                                            "description": "Destination path in runtime stage"
                                        }
                                    },
                                    "required": ["from", "to"]
                                },
                                "minItems": 1
                            },
                            "command": {
                                "type": "array",
                                "description": "Container entrypoint command as array (e.g., ['/usr/local/bin/myapp'] or ['python', 'main.py'])",
                                "items": {
                                    "type": "string"
                                },
                                "minItems": 1
                            },
                            "ports": {
                                "type": "array",
                                "description": "Ports to expose (e.g., [8080, 8443])",
                                "items": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "maximum": 65535
                                },
                                "default": []
                            },
                            "healthcheck": {
                                "type": "object",
                                "description": "Optional health check configuration for services",
                                "properties": {
                                    "test": {
                                        "type": "array",
                                        "description": "Health check command (e.g., ['CMD', 'curl', '-f', 'http://localhost/health'])",
                                        "items": {
                                            "type": "string"
                                        },
                                        "minItems": 1
                                    },
                                    "interval": {
                                        "type": "string",
                                        "description": "Interval between checks (e.g., '30s')"
                                    },
                                    "timeout": {
                                        "type": "string",
                                        "description": "Timeout for each check (e.g., '3s')"
                                    },
                                    "retries": {
                                        "type": "integer",
                                        "description": "Number of consecutive failures before unhealthy",
                                        "minimum": 1
                                    }
                                },
                                "required": ["test"]
                            }
                        },
                        "required": ["base", "copy", "command"]
                    }
                },
                "required": ["version", "metadata", "build", "runtime"]
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
            assert!(tool.schema.is_some(), "Tool {} missing schema", tool.name);
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

        let required_top_level = ["version", "metadata", "build", "runtime"];
        for field in &required_top_level {
            assert!(
                schema["properties"][field].is_object(),
                "Missing top-level property: {}",
                field
            );
        }

        let required_array = schema["required"].as_array().unwrap();
        assert_eq!(required_array.len(), 4);

        assert_eq!(schema["properties"]["version"]["type"], "string");
        assert_eq!(schema["properties"]["metadata"]["type"], "object");
        assert_eq!(schema["properties"]["build"]["type"], "object");
        assert_eq!(schema["properties"]["runtime"]["type"], "object");
    }

    #[test]
    fn test_schema_types() {
        let tool = ToolRegistry::create_submit_detection_tool();
        let schema = tool.schema.unwrap();

        let metadata = &schema["properties"]["metadata"];
        assert_eq!(metadata["properties"]["language"]["type"], "string");
        assert_eq!(metadata["properties"]["build_system"]["type"], "string");
        assert_eq!(metadata["properties"]["confidence"]["type"], "number");
        assert_eq!(metadata["properties"]["reasoning"]["type"], "string");

        let build = &schema["properties"]["build"];
        assert_eq!(build["properties"]["base"]["type"], "string");
        assert_eq!(build["properties"]["commands"]["type"], "array");
        assert_eq!(build["properties"]["context"]["type"], "array");
        assert_eq!(build["properties"]["artifacts"]["type"], "array");

        let runtime = &schema["properties"]["runtime"];
        assert_eq!(runtime["properties"]["base"]["type"], "string");
        assert_eq!(runtime["properties"]["copy"]["type"], "array");
        assert_eq!(runtime["properties"]["command"]["type"], "array");
    }

    #[test]
    fn test_confidence_constraints() {
        let tool = ToolRegistry::create_submit_detection_tool();
        let schema = tool.schema.unwrap();

        let confidence_schema = &schema["properties"]["metadata"]["properties"]["confidence"];
        assert_eq!(confidence_schema["minimum"], 0.0);
        assert_eq!(confidence_schema["maximum"], 1.0);
    }
}
