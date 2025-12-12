//! Tool registry
//!
//! Maintains a registry of all available tools and provides them to the LLM.

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

use crate::languages::LanguageRegistry;
use crate::llm::ToolDefinition;
use super::trait_def::Tool;
use super::implementations::*;

/// Registry of all available tools
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new registry with all standard tools for the given repository
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        let language_registry = Arc::new(LanguageRegistry::with_defaults());

        let tools: Vec<Arc<dyn Tool>> = vec![
            Arc::new(ListFilesTool::new(repo_path.clone(), Arc::clone(&language_registry))?),
            Arc::new(ReadFileTool::new(repo_path.clone(), Arc::clone(&language_registry))?),
            Arc::new(SearchFilesTool::new(repo_path.clone(), Arc::clone(&language_registry))?),
            Arc::new(GetFileTreeTool::new(repo_path.clone(), Arc::clone(&language_registry))?),
            Arc::new(GrepContentTool::new(repo_path, Arc::clone(&language_registry))?),
            Arc::new(GetBestPracticesTool::new(Arc::clone(&language_registry))),
            Arc::new(SubmitDetectionTool),
        ];

        Ok(Self { tools })
    }

    /// Get all tools as ToolDefinition for LLMClient trait
    pub fn as_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .iter()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.schema(),
            })
            .collect()
    }

    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .cloned()
    }

    /// Get all registered tool names
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name()).collect()
    }

    /// Number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_registry_creation() {
        let temp_dir = TempDir::new().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf()).unwrap();

        assert_eq!(registry.len(), 7);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_tool_names() {
        let temp_dir = TempDir::new().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf()).unwrap();

        let names = registry.tool_names();
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"search_files"));
        assert!(names.contains(&"get_file_tree"));
        assert!(names.contains(&"grep_content"));
        assert!(names.contains(&"get_best_practices"));
        assert!(names.contains(&"submit_detection"));
    }

    #[test]
    fn test_get_tool() {
        let temp_dir = TempDir::new().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf()).unwrap();

        let tool = registry.get_tool("list_files");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name(), "list_files");

        let missing = registry.get_tool("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_as_tool_definitions() {
        let temp_dir = TempDir::new().unwrap();
        let registry = ToolRegistry::new(temp_dir.path().to_path_buf()).unwrap();

        let definitions = registry.as_tool_definitions();
        assert_eq!(definitions.len(), 7);

        for def in definitions {
            assert!(!def.name.is_empty());
            assert!(!def.description.is_empty());
            assert!(!def.parameters.is_null());
        }
    }
}
