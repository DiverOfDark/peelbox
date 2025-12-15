// Pipeline phases for multi-step build system detection
//
// This module contains the phase-based detection pipeline that replaces
// the tool-based agentic loop. Each phase is self-contained with its own
// prompt builder and execution logic.

pub mod llm_helper;
pub mod extractor_helper;

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
#[path = "06_runtime.rs"]
pub mod runtime;
#[path = "07_build.rs"]
pub mod build;
#[path = "08_entrypoint.rs"]
pub mod entrypoint;
#[path = "09_native_deps.rs"]
pub mod native_deps;
#[path = "10_port.rs"]
pub mod port;
#[path = "11_env_vars.rs"]
pub mod env_vars;
#[path = "12_health.rs"]
pub mod health;
#[path = "13_cache.rs"]
pub mod cache;
#[path = "14_root_cache.rs"]
pub mod root_cache;
#[path = "15_assemble.rs"]
pub mod assemble;
