//! Bootstrap scanner for pre-LLM repository analysis
//!
//! This module provides fast repository scanning using the LanguageRegistry
//! to detect build systems and generate context for LLM prompts.

mod context;
mod scanner;

pub use context::{BootstrapContext, LanguageDetection, RepoSummary, WorkspaceInfo};
pub use scanner::BootstrapScanner;
