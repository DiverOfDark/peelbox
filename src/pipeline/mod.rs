//! Analysis pipeline orchestration
//!
//! This module provides the core pipeline infrastructure for coordinating
//! build system detection through iterative LLM conversation.

pub mod config;
pub mod context;

pub use config::PipelineConfig;
pub use context::PipelineContext;
