//! Tool execution system for LLM-based repository analysis
//!
//! This module provides a trait-based tool system that the LLM can use to
//! iteratively explore repositories and gather information for build detection.

pub mod cache;
pub mod implementations;
pub mod registry;
pub mod system;
pub mod trait_def;

pub use cache::ToolCache;
pub use registry::ToolRegistry;
pub use system::ToolSystem;
pub use trait_def::Tool;
