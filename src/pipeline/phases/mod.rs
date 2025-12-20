// Pipeline phases for multi-step build system detection
//
// This module contains the phase-based detection pipeline that replaces
// the tool-based agentic loop. Each phase is self-contained with its own
// prompt builder and execution logic.

pub mod extractor_helper;
pub mod llm_helper;

#[path = "01_scan.rs"]
pub mod scan;
#[path = "02_classify.rs"]
pub mod classify;
#[path = "03_structure.rs"]
pub mod structure;
#[path = "04_dependencies.rs"]
pub mod dependencies;
#[path = "05_build_order.rs"]
pub mod build_order;
#[path = "06_root_cache.rs"]
pub mod root_cache;
#[path = "07_service_analysis.rs"]
pub mod service_analysis;
#[path = "08_assemble.rs"]
pub mod assemble;

// Service phases (executed within ServiceAnalysisPhase)
#[path = "07_1_runtime.rs"]
pub mod runtime;
#[path = "07_2_runtime_config.rs"]
pub mod runtime_config;
#[path = "07_2_build.rs"]
pub mod build;
#[path = "07_8_cache.rs"]
pub mod cache;
