// Code-based extraction for runtime configuration
//
// Extractors analyze source code and configuration files to extract
// runtime information like ports, environment variables, and health endpoints
// without requiring LLM inference.

pub mod context;
pub mod env_vars;
pub mod health;
pub mod parsers;
pub mod port;
pub mod registry;

pub use context::ServiceContext;
pub use env_vars::{EnvVarExtractor, EnvVarInfo, EnvVarSource};
pub use health::{HealthCheckExtractor, HealthCheckInfo, HealthCheckSource};
pub use port::{PortExtractor, PortInfo, PortSource};
pub use registry::ExtractorRegistry;
