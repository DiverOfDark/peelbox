//! Parser and detector registry.
//!
//! Uses `inventory` for automatic registration. Each parser/detector file
//! submits an entry via `inventory::submit!` so adding a new parser is
//! fully self-contained — no manual list to update.

use crate::traits::{ConfigParser, FrameworkDetector, ManifestParser};

/// Entry for auto-registering a manifest parser.
pub struct ManifestParserEntry(pub fn() -> Box<dyn ManifestParser>);
inventory::collect!(ManifestParserEntry);

/// Entry for auto-registering a config parser.
pub struct ConfigParserEntry(pub fn() -> Box<dyn ConfigParser>);
inventory::collect!(ConfigParserEntry);

/// Entry for auto-registering a framework detector.
pub struct FrameworkDetectorEntry(pub fn() -> Box<dyn FrameworkDetector>);
inventory::collect!(FrameworkDetectorEntry);

/// Central registry of all parsers and detectors.
pub struct Registry {
    pub manifest_parsers: Vec<Box<dyn ManifestParser>>,
    pub config_parsers: Vec<Box<dyn ConfigParser>>,
    pub framework_detectors: Vec<Box<dyn FrameworkDetector>>,
}

impl Registry {
    /// Build a registry with all built-in parsers and detectors.
    pub fn with_defaults() -> Self {
        Self {
            manifest_parsers: inventory::iter::<ManifestParserEntry>
                .into_iter()
                .map(|e| (e.0)())
                .collect(),
            config_parsers: inventory::iter::<ConfigParserEntry>
                .into_iter()
                .map(|e| (e.0)())
                .collect(),
            framework_detectors: inventory::iter::<FrameworkDetectorEntry>
                .into_iter()
                .map(|e| (e.0)())
                .collect(),
        }
    }
}
