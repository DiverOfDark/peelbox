//! Tool trait definition
//!
//! Defines the interface that all tools must implement for LLM-based detection.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// A tool that the LLM can call to gather information about the repository
#[async_trait]
pub trait Tool: Send + Sync {
    /// Unique name of the tool (e.g., "list_files", "read_file")
    fn name(&self) -> &'static str;

    /// Human-readable description of what the tool does
    fn description(&self) -> &'static str;

    /// JSON Schema describing the tool's parameters
    fn schema(&self) -> Value;

    /// Execute the tool with the given arguments
    ///
    /// # Arguments
    /// * `arguments` - JSON object containing tool parameters
    ///
    /// # Returns
    /// String result that will be sent back to the LLM
    async fn execute(&self, arguments: Value) -> Result<String>;
}
