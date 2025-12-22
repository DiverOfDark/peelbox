//! Unified stack registry for languages, build systems, frameworks, and orchestrators.
//!
//! Type-safe registry system with strongly-typed identifiers (LanguageId, BuildSystemId,
//! FrameworkId, OrchestratorId) providing compile-time validation.
//!
//! # Example
//!
//! ```no_run
//! use aipack::stack::{StackRegistry, BuildSystemId, LanguageId};
//! use std::path::Path;
//!
//! # fn main() -> anyhow::Result<()> {
//! let registry = StackRegistry::with_defaults();
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

pub mod buildsystem;
pub mod build_system_id;
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
