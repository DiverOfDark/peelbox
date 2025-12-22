//! Unified stack registry for languages, build systems, frameworks, and orchestrators.
//!
//! Type-safe registry system with strongly-typed identifiers (LanguageId, BuildSystemId,
//! FrameworkId, OrchestratorId, RuntimeId) providing compile-time validation.
//!
//! # Custom Variants
//!
//! All ID enums support a `Custom(String)` variant to represent technologies discovered
//! dynamically via LLM inference when deterministic pattern matching fails. This enables
//! detection of unknown or emerging technologies without code changes.
//!
//! # Detection Strategy
//!
//! The registry uses a two-tier detection strategy:
//! 1. **Deterministic First**: Pattern-based detection for known technologies (fast, reliable)
//! 2. **LLM Fallback**: When patterns fail, LLM-backed implementations discover unknown tech
//!
//! LLM fallback is automatic when an LLM client is provided to `StackRegistry::with_defaults()`.
//!
//! # Example
//!
//! ```no_run
//! use aipack::stack::{StackRegistry, BuildSystemId, LanguageId};
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! let registry = StackRegistry::with_defaults(None);
//!
//! let manifest_path = Path::new("Cargo.toml");
//! let content = std::fs::read_to_string(manifest_path)?;
//! let stack = registry.detect_stack(manifest_path, &content).unwrap();
//!
//! let build_system = registry.get_build_system(BuildSystemId::Cargo).unwrap();
//! let language = registry.get_language(LanguageId::Rust).unwrap();
//! # Ok(())
//! # }
//! ```

#[macro_use]
pub mod id_enum_macro;

pub mod build_system_id;
pub mod buildsystem;
pub mod detection;
pub mod framework;
pub mod framework_id;
pub mod language;
pub mod language_id;
pub mod orchestrator;
pub mod registry;
pub mod runtime;
pub mod runtime_id;

pub use build_system_id::BuildSystemId;
pub use buildsystem::{BuildSystem, BuildTemplate, ManifestPattern};
pub use detection::DetectionStack;
pub use framework::{DependencyPattern, DependencyPatternType, Framework};
pub use framework_id::FrameworkId;
pub use language::{
    Dependency, DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition,
};
pub use language_id::LanguageId;
pub use orchestrator::{MonorepoOrchestrator, OrchestratorId};
pub use registry::StackRegistry;
pub use runtime_id::RuntimeId;
