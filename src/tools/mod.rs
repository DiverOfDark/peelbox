//! Tool execution system for LLM-based repository analysis
//!
//! This module provides a trait-based tool system that the LLM can use to
//! iteratively explore repositories and gather information for build detection.

pub mod trait_def;
pub mod registry;
pub mod cache;
pub mod system;
pub mod implementations;
pub mod best_practices;

pub use trait_def::Tool;
pub use registry::ToolRegistry;
pub use cache::ToolCache;
pub use system::ToolSystem;
pub use best_practices::BestPractices;
