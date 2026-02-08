//! Language profile: static data about a programming language
//!
//! Replaces ~10 trait methods that returned constant data with a single
//! struct that can be accessed via `LanguageDefinition::profile()`.

use crate::RuntimeId;

/// Static metadata about a programming language.
///
/// Replaces individual trait methods like `extensions()`, `excluded_dirs()`,
/// `runtime_name()`, `default_port()`, etc. with a single data struct.
#[derive(Debug, Clone)]
pub struct LanguageProfile {
    /// File extensions for this language (without dots)
    pub extensions: &'static [&'static str],
    /// Directories to exclude during scanning
    pub excluded_dirs: &'static [&'static str],
    /// Runtime identifier for this language
    pub runtime_id: RuntimeId,
    /// Default port for web servers in this language
    pub default_port: Option<u16>,
    /// Patterns to detect environment variable usage: (regex, label)
    pub env_var_patterns: &'static [(&'static str, &'static str)],
    /// Patterns to detect port configuration: (regex, label)
    pub port_patterns: &'static [(&'static str, &'static str)],
    /// Patterns to detect health check endpoints: (regex, label)
    pub health_check_patterns: &'static [(&'static str, &'static str)],
    /// Default health check endpoints when none detected: (endpoint, framework)
    pub default_health_endpoints: &'static [(&'static str, &'static str)],
}
