use anyhow::{anyhow, Result};
use serde_json::Value;
use std::path::PathBuf;
use tracing::{debug, info, warn};

use super::cache::ToolCache;
use super::registry::ToolRegistry;
use crate::llm::ToolDefinition;

pub struct ToolSystem {
    registry: ToolRegistry,
    cache: ToolCache,
}

impl ToolSystem {
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        Ok(Self {
            registry: ToolRegistry::new(repo_path)?,
            cache: ToolCache::new(),
        })
    }

    /// Execute a tool and return structured JSON result
    pub async fn execute(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        info!(tool = tool_name, args = ?arguments, "Executing tool");

        if let Some(cached) = self.cache.get(tool_name, &arguments) {
            debug!(tool = tool_name, "Tool result found in cache");
            return Ok(cached);
        }

        let tool = self
            .registry
            .get_tool(tool_name)
            .ok_or_else(|| anyhow!("Unknown tool: {}", tool_name))?;

        let result = tool.execute(arguments.clone()).await;

        match &result {
            Ok(output) => {
                let output_preview = serde_json::to_string(output).unwrap_or_default();
                let preview_len = output_preview.len().min(200);
                info!(tool = tool_name, "Tool execution completed");
                debug!(
                    tool = tool_name,
                    output_preview = &output_preview[..preview_len],
                    "Tool output preview"
                );

                self.cache.insert(tool_name, &arguments, output.clone());
            }
            Err(e) => {
                warn!(tool = tool_name, error = %e, "Tool execution failed");
            }
        }

        result
    }

    pub fn as_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.registry.as_tool_definitions()
    }

    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    pub fn tool_names(&self) -> Vec<&str> {
        self.registry.tool_names()
    }

    pub fn tool_count(&self) -> usize {
        self.registry.len()
    }

    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let base = dir.path();

        fs::create_dir(base.join("src")).unwrap();
        fs::write(base.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(base.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();

        dir
    }

    #[tokio::test]
    async fn test_tool_system_creation() {
        let temp_dir = create_test_repo();
        let system = ToolSystem::new(temp_dir.path().to_path_buf()).unwrap();

        assert_eq!(system.tool_count(), 7);
        assert_eq!(system.cache_size(), 0);
    }

    #[tokio::test]
    async fn test_execute_list_files() {
        let temp_dir = create_test_repo();
        let system = ToolSystem::new(temp_dir.path().to_path_buf()).unwrap();

        let result = system
            .execute("list_files", json!({"path": "."}))
            .await
            .unwrap();

        let result_str = serde_json::to_string(&result).unwrap();
        assert!(result_str.contains("Cargo.toml"));
        assert!(result_str.contains("src/main.rs"));
    }

    #[tokio::test]
    async fn test_execute_read_file() {
        let temp_dir = create_test_repo();
        let system = ToolSystem::new(temp_dir.path().to_path_buf()).unwrap();

        let result = system
            .execute("read_file", json!({"path": "Cargo.toml"}))
            .await
            .unwrap();

        let result_str = serde_json::to_string(&result).unwrap();
        assert!(result_str.contains("[package]"));
        assert!(result_str.contains("name = \\\"test\\\""));
    }

    #[tokio::test]
    async fn test_caching() {
        let temp_dir = create_test_repo();
        let system = ToolSystem::new(temp_dir.path().to_path_buf()).unwrap();

        assert_eq!(system.cache_size(), 0);

        let args = json!({"path": "."});
        let result1 = system.execute("list_files", args.clone()).await.unwrap();

        assert_eq!(system.cache_size(), 1);

        let result2 = system.execute("list_files", args).await.unwrap();

        assert_eq!(result1, result2);
        assert_eq!(system.cache_size(), 1);
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let temp_dir = create_test_repo();
        let system = ToolSystem::new(temp_dir.path().to_path_buf()).unwrap();

        system
            .execute("list_files", json!({"path": "."}))
            .await
            .unwrap();

        assert_eq!(system.cache_size(), 1);

        system.clear_cache();

        assert_eq!(system.cache_size(), 0);
    }

    #[tokio::test]
    async fn test_unknown_tool() {
        let temp_dir = create_test_repo();
        let system = ToolSystem::new(temp_dir.path().to_path_buf()).unwrap();

        let result = system.execute("nonexistent", json!({})).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown tool"));
    }

    #[tokio::test]
    async fn test_tool_names() {
        let temp_dir = create_test_repo();
        let system = ToolSystem::new(temp_dir.path().to_path_buf()).unwrap();

        let names = system.tool_names();
        assert_eq!(names.len(), 7);
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"submit_detection"));
    }

    #[tokio::test]
    async fn test_as_tool_definitions() {
        let temp_dir = create_test_repo();
        let system = ToolSystem::new(temp_dir.path().to_path_buf()).unwrap();

        let definitions = system.as_tool_definitions();
        assert_eq!(definitions.len(), 7);

        for def in definitions {
            assert!(!def.name.is_empty());
            assert!(!def.description.is_empty());
        }
    }
}
