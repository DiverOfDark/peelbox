use crate::stack::framework::Framework;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod jvm;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub entrypoint: Option<String>,
    pub port: Option<u16>,
    pub env_vars: Vec<String>,
    pub health: Option<HealthCheck>,
    pub native_deps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub endpoint: String,
}

#[async_trait]
pub trait Runtime: Send + Sync {
    fn name(&self) -> &str;

    /// Try deterministic configuration extraction first (parse known files)
    fn try_deterministic_config(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig>;

    /// Fallback to LLM-based configuration extraction
    async fn extract_config_llm(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Result<RuntimeConfig>;

    /// Get runtime base image with optional version
    fn runtime_base_image(&self, version: Option<&str>) -> String;

    /// Get required system packages
    fn required_packages(&self) -> Vec<&str>;

    /// Generate start command for the given entrypoint
    fn start_command(&self, entrypoint: &Path) -> String;
}
