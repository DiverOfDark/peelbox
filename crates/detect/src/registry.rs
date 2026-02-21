//! Parser, detector, and profile registry.
//!
//! Uses `inventory` for automatic registration. Each parser/detector/profile file
//! submits an entry via `inventory::submit!` so adding a new parser is
//! fully self-contained — no manual list to update.

use crate::ids::BuildSystemId;
use crate::traits::{ConfigParser, FrameworkDetector, ManifestParser};
use crate::types::BuildSystemConfig;
use std::collections::HashMap;

/// Entry for auto-registering a manifest parser.
pub struct ManifestParserEntry(pub fn() -> Box<dyn ManifestParser>);
inventory::collect!(ManifestParserEntry);

/// Entry for auto-registering a config parser.
pub struct ConfigParserEntry(pub fn() -> Box<dyn ConfigParser>);
inventory::collect!(ConfigParserEntry);

/// Entry for auto-registering a framework detector.
pub struct FrameworkDetectorEntry(pub fn() -> Box<dyn FrameworkDetector>);
inventory::collect!(FrameworkDetectorEntry);

/// Entry for auto-registering a build system profile.
pub struct BuildSystemProfileEntry(pub fn() -> BuildSystemConfig);
inventory::collect!(BuildSystemProfileEntry);

/// Central registry of all parsers, detectors, and profiles.
pub struct Registry {
    pub manifest_parsers: Vec<Box<dyn ManifestParser>>,
    pub config_parsers: Vec<Box<dyn ConfigParser>>,
    pub framework_detectors: Vec<Box<dyn FrameworkDetector>>,
    pub build_system_profiles: HashMap<BuildSystemId, BuildSystemConfig>,
}

impl Registry {
    /// Build a registry with all built-in parsers, detectors, and profiles.
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
            build_system_profiles: inventory::iter::<BuildSystemProfileEntry>
                .into_iter()
                .map(|e| {
                    let config = (e.0)();
                    (config.id, config)
                })
                .collect(),
        }
    }

    /// Look up a build system profile by ID.
    pub fn get_profile(&self, id: &BuildSystemId) -> Option<&BuildSystemConfig> {
        self.build_system_profiles.get(id)
    }
}
