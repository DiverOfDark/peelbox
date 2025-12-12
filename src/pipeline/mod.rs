//! Analysis pipeline orchestration
//!
//! This module provides the core pipeline infrastructure for coordinating
//! build system detection through iterative LLM conversation.

pub mod analysis;
pub mod config;
pub mod context;

pub use analysis::{AnalysisPipeline, PipelineError};
pub use config::PipelineConfig;
pub use context::PipelineContext;
