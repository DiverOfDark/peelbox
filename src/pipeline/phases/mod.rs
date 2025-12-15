// Pipeline phases for multi-step build system detection
//
// This module contains the phase-based detection pipeline that replaces
// the tool-based agentic loop. Each phase is self-contained with its own
// prompt builder and execution logic.

pub mod llm_helper;
pub mod scan;
pub mod classify;
pub mod structure;
pub mod dependencies;
pub mod build_order;
pub mod runtime;
pub mod build;
pub mod entrypoint;
pub mod native_deps;
pub mod port;
pub mod env_vars;
pub mod health;
pub mod cache;
pub mod root_cache;
pub mod assemble;
