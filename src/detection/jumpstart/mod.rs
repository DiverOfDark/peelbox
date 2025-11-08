//! Jumpstart analysis for rapid build system detection
//!
//! This module provides pre-scanning of repositories to identify manifest files
//! before LLM analysis, dramatically reducing the number of LLM tool calls and
//! improving detection speed.

pub mod context;
pub mod patterns;
pub mod scanner;

pub use context::JumpstartContext;
pub use scanner::JumpstartScanner;
