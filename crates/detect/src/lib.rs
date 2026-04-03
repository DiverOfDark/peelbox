//! Map-reduce detection pipeline for peelbox.
//!
//! This crate replaces the previous multi-phase pipeline with a simpler four-step approach:
//! 1. **Parse** — Walk filesystem, parse each file into a typed tree (Manifest | ConfigContribution | Other)
//! 2. **Detect Framework** — Check each manifest's dependencies against framework detectors
//! 3. **Partition** — Group manifests + configs into service buckets by workspace topology
//! 4. **Reduce** — Merge each service bucket into a `UniversalBuild`
//!
//! All manifest formats normalize into one generic `Manifest` type. No confidence scores.
//! Either a file parses successfully or it doesn't. Binary, deterministic.

pub mod framework_detectors;
pub mod helpers;
pub mod ids;
pub mod parsers;
pub mod pipeline;
pub mod registry;
pub mod source_scanning;
pub mod traits;
pub mod types;

pub use pipeline::{
    detect, detect_with_registry, detect_with_registry_and_wolfi, detect_without_wolfi,
};
pub use registry::Registry;
pub use types::BuildSystemConfig;
