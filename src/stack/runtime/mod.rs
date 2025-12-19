use crate::stack::framework::Framework;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod beam;
pub mod dotnet;
pub mod jvm;
pub mod llm;
pub mod native;
pub mod node;
pub mod php;
pub mod python;
pub mod ruby;

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

pub trait Runtime: Send + Sync {
    fn name(&self) -> &str;

    /// Try to extract runtime configuration (parse known files)
    /// Returns None if config cannot be extracted deterministically
    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig>;

    /// Get runtime base image with optional version
    fn runtime_base_image(&self, version: Option<&str>) -> String;

    /// Get required system packages
    fn required_packages(&self) -> Vec<&str>;

    /// Generate start command for the given entrypoint
    fn start_command(&self, entrypoint: &Path) -> String;
}

pub use beam::BeamRuntime;
pub use dotnet::DotNetRuntime;
pub use jvm::JvmRuntime;
pub use llm::LLMRuntime;
pub use native::NativeRuntime;
pub use node::NodeRuntime;
pub use php::PhpRuntime;
pub use python::PythonRuntime;
pub use ruby::RubyRuntime;
