//! Parser and detector trait definitions.

use crate::ids::{FrameworkId, LanguageId};
use crate::types::{ConfigContribution, Dependency, FrameworkContribution, Manifest};
use std::path::Path;

/// Parses a manifest file into a normalized Manifest.
pub trait ManifestParser: Send + Sync {
    /// Filenames this parser handles (e.g., ["Cargo.toml"]).
    fn filenames(&self) -> &[&str];

    /// Parse and normalize. Returns None if content doesn't match.
    fn parse(&self, path: &Path, content: &str) -> Option<Manifest>;
}

/// Parses a config file into a ConfigContribution.
pub trait ConfigParser: Send + Sync {
    /// Filenames this parser handles.
    fn filenames(&self) -> &[&str];

    /// Parse and normalize. Returns None if content doesn't match.
    fn parse(&self, path: &Path, content: &str) -> Option<ConfigContribution>;
}

/// Detects a framework from dependencies and contributes additional config.
pub trait FrameworkDetector: Send + Sync {
    fn id(&self) -> FrameworkId;
    fn compatible_languages(&self) -> &[LanguageId];

    /// Check if these dependencies indicate this framework.
    fn detect(&self, deps: &[Dependency]) -> bool;

    /// Additional configuration when this framework is detected.
    /// Receives the dependency list so detectors can conditionally adjust contributions.
    fn contribution(&self, deps: &[Dependency]) -> FrameworkContribution;
}
