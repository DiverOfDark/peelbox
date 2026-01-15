// Pipeline phases for multi-step build system detection
//
// This module contains the phase-based detection pipeline that replaces
// the tool-based agentic loop. Each phase is self-contained with its own
// prompt builder and execution logic.

pub mod extractor_helper;

#[path = "08_assemble.rs"]
pub mod assemble;
#[path = "06_root_cache.rs"]
pub mod root_cache;
#[path = "01_scan.rs"]
pub mod scan;
#[path = "07_service_analysis.rs"]
pub mod service_analysis;
#[path = "02_workspace.rs"]
pub mod workspace;

// Service phases (executed within ServiceAnalysisPhase)
#[path = "07_2_build.rs"]
pub mod build;
#[path = "07_8_cache.rs"]
pub mod cache;
#[path = "07_2_runtime_config.rs"]
pub mod runtime_config;
#[path = "07_0_stack.rs"]
pub mod stack;
