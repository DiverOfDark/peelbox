// Code-based extraction for runtime configuration
//
// Extractors analyze source code and configuration files to extract
// runtime information like ports, environment variables, and health endpoints
// without requiring LLM inference.

pub mod env_vars;
pub mod health;
pub mod port;
pub mod registry;

pub use registry::ExtractorRegistry;
