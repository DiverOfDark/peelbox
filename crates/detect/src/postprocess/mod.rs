//! Language-specific post-processing passes.
//!
//! These run after the main reduce step to fix up entrypoints, detect native
//! dependencies, and sanitize build commands for specific ecosystems.

pub mod framework;
pub mod node;
pub mod python;
