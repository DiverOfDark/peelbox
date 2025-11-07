//! Utility modules for aipack
//!
//! This module provides various utility functions and helpers including:
//! - Structured logging setup and configuration
//! - File system operations (future)
//! - Caching utilities (future)

pub mod logging;

// Re-export commonly used items
pub use logging::{init_default, init_from_env, init_logging, LoggingConfig};
