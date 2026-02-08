//! Shared helpers used across parsers and detectors.

use std::collections::BTreeMap;

/// Build a `BTreeMap<String, String>` from a slice of `(&str, &str)` pairs.
pub fn btree(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}
