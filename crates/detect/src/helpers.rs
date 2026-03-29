//! Shared helpers used across parsers and detectors.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Build a `BTreeMap<String, String>` from a slice of `(&str, &str)` pairs.
pub fn btree(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Extract the project directory from the build metadata reasoning string.
pub fn extract_project_dir(repo_root: &Path, reasoning: &str) -> PathBuf {
    if let Some(in_pos) = reasoning.rfind(" in ") {
        let dir = &reasoning[in_pos + 4..];
        repo_root.join(dir)
    } else {
        repo_root.to_path_buf()
    }
}

/// Replace an unversioned package with a versioned one.
/// Only replaces exact matches (not already-versioned packages).
pub fn replace_package(packages: &mut [String], unversioned: &str, versioned: &str) {
    for pkg in packages.iter_mut() {
        if pkg == unversioned {
            *pkg = versioned.to_string();
        }
    }
}

/// Extract major version from a version string (e.g., "22.1.0" -> "22", "24" -> "24").
pub fn extract_major_version(version: &str) -> Option<&str> {
    let version = version.trim();
    if version == "latest" || version == "lts" {
        return None;
    }
    Some(version.split('.').next().unwrap_or(version))
}

/// Extract major.minor version from a version string (e.g., "3.12.1" -> "3.12").
pub fn extract_major_minor_version(version: &str) -> Option<String> {
    let version = version.trim();
    if version == "latest" || version == "lts" {
        return None;
    }
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        Some(format!("{}.{}", parts[0], parts[1]))
    } else {
        None
    }
}
