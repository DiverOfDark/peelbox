//! Unified stack registry for languages, build systems, frameworks, and orchestrators.
//!
//! Type-safe registry with strongly-typed IDs. All ID enums support `Custom(String)` variant
//! for LLM-discovered technologies. Detection uses deterministic patterns first, LLM fallback
//! when patterns fail.

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
