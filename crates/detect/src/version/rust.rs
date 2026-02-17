//! Rust version detection from rust-toolchain.toml, rust-toolchain, and Cargo.toml rust-version.
//!
//! Priority order:
//! 1. `rust-toolchain.toml` — `[toolchain] channel = "1.75.0"`
//! 2. `rust-toolchain` — plain text channel string (e.g., `1.75.0`, `stable`, `nightly`)
//! 3. `Cargo.toml` — `rust-version = "1.70"` (MSRV)

use std::path::Path;

/// Read Rust version from toolchain/version files in the project directory and repo root.
/// Returns the major.minor version string (e.g., "1.75").
pub fn read_rust_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    for dir in &[project_dir, repo_root] {
        // 1. rust-toolchain.toml (highest priority)
        let toolchain_toml = dir.join("rust-toolchain.toml");
        if let Ok(content) = std::fs::read_to_string(&toolchain_toml) {
            if let Some(version) = parse_rust_toolchain_toml(&content) {
                return Some(version);
            }
        }

        // 2. rust-toolchain (plain text)
        let toolchain = dir.join("rust-toolchain");
        if let Ok(content) = std::fs::read_to_string(&toolchain) {
            if let Some(version) = parse_rust_toolchain(&content) {
                return Some(version);
            }
        }
    }

    // 3. Cargo.toml rust-version (MSRV) — lowest priority
    for dir in &[project_dir, repo_root] {
        let cargo_toml = dir.join("Cargo.toml");
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            if let Some(version) = parse_cargo_rust_version(&content) {
                return Some(version);
            }
        }
    }

    None
}

/// Parse `rust-toolchain.toml` content to extract the channel version.
/// Supports both TOML format `[toolchain] channel = "1.75.0"` and plain `channel = "..."`.
fn parse_rust_toolchain_toml(content: &str) -> Option<String> {
    let toml_val: toml::Value = toml::from_str(content).ok()?;

    // Try [toolchain].channel first
    let channel = toml_val
        .get("toolchain")
        .and_then(|t| t.get("channel"))
        .and_then(|c| c.as_str())?;

    extract_version_from_channel(channel)
}

/// Parse plain `rust-toolchain` file content.
/// Can be either a plain channel string or TOML format.
fn parse_rust_toolchain(content: &str) -> Option<String> {
    let trimmed = content.trim();

    // Try TOML format first (some rust-toolchain files use TOML)
    if trimmed.contains("[toolchain]") || trimmed.contains("channel") {
        return parse_rust_toolchain_toml(trimmed);
    }

    // Plain text channel string
    extract_version_from_channel(trimmed)
}

/// Parse `rust-version` field from Cargo.toml content.
fn parse_cargo_rust_version(content: &str) -> Option<String> {
    let toml_val: toml::Value = toml::from_str(content).ok()?;

    let rust_version = toml_val
        .get("package")
        .and_then(|p| p.get("rust-version"))
        .and_then(|v| v.as_str())?;

    extract_major_minor(rust_version)
}

/// Extract major.minor version from a Rust channel string.
/// Examples: "1.75.0" → "1.75", "1.70" → "1.70", "stable" → None, "nightly" → None
fn extract_version_from_channel(channel: &str) -> Option<String> {
    let channel = channel.trim();

    // Skip non-version channels
    if channel == "stable" || channel == "beta" || channel.starts_with("nightly") {
        return None;
    }

    extract_major_minor(channel)
}

/// Extract major.minor from a version string.
/// "1.75.0" → "1.75", "1.70" → "1.70", "1.80.1" → "1.80"
fn extract_major_minor(version: &str) -> Option<String> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
    {
        Some(format!("{}.{}", parts[0], parts[1]))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rust_toolchain_toml() {
        let content = r#"
[toolchain]
channel = "1.75.0"
components = ["rustfmt", "clippy"]
"#;
        assert_eq!(parse_rust_toolchain_toml(content), Some("1.75".to_string()));
    }

    #[test]
    fn test_parse_rust_toolchain_toml_no_patch() {
        let content = r#"
[toolchain]
channel = "1.75"
"#;
        assert_eq!(parse_rust_toolchain_toml(content), Some("1.75".to_string()));
    }

    #[test]
    fn test_parse_rust_toolchain_toml_stable() {
        let content = r#"
[toolchain]
channel = "stable"
"#;
        assert_eq!(parse_rust_toolchain_toml(content), None);
    }

    #[test]
    fn test_parse_rust_toolchain_toml_nightly() {
        let content = r#"
[toolchain]
channel = "nightly-2024-01-01"
"#;
        assert_eq!(parse_rust_toolchain_toml(content), None);
    }

    #[test]
    fn test_parse_rust_toolchain_plain() {
        assert_eq!(parse_rust_toolchain("1.75.0\n"), Some("1.75".to_string()));
    }

    #[test]
    fn test_parse_rust_toolchain_plain_stable() {
        assert_eq!(parse_rust_toolchain("stable\n"), None);
    }

    #[test]
    fn test_parse_rust_toolchain_toml_in_plain_file() {
        let content = r#"
[toolchain]
channel = "1.80.1"
"#;
        assert_eq!(parse_rust_toolchain(content), Some("1.80".to_string()));
    }

    #[test]
    fn test_parse_cargo_rust_version() {
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"
rust-version = "1.70"
"#;
        assert_eq!(parse_cargo_rust_version(content), Some("1.70".to_string()));
    }

    #[test]
    fn test_parse_cargo_rust_version_with_patch() {
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"
rust-version = "1.70.0"
"#;
        assert_eq!(parse_cargo_rust_version(content), Some("1.70".to_string()));
    }

    #[test]
    fn test_parse_cargo_no_rust_version() {
        let content = r#"
[package]
name = "my-app"
version = "0.1.0"
"#;
        assert_eq!(parse_cargo_rust_version(content), None);
    }

    #[test]
    fn test_extract_major_minor() {
        assert_eq!(extract_major_minor("1.75.0"), Some("1.75".to_string()));
        assert_eq!(extract_major_minor("1.70"), Some("1.70".to_string()));
        assert_eq!(extract_major_minor("1.80.1"), Some("1.80".to_string()));
        assert_eq!(extract_major_minor("1"), None);
        assert_eq!(extract_major_minor("stable"), None);
    }

    #[test]
    fn test_read_rust_version_from_toolchain_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.75.0\"\n",
        )
        .unwrap();

        assert_eq!(
            read_rust_version(dir.path(), dir.path()),
            Some("1.75".to_string())
        );
    }

    #[test]
    fn test_read_rust_version_from_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nrust-version = \"1.70\"\n",
        )
        .unwrap();

        assert_eq!(
            read_rust_version(dir.path(), dir.path()),
            Some("1.70".to_string())
        );
    }

    #[test]
    fn test_toolchain_toml_takes_precedence_over_cargo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.75.0\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nrust-version = \"1.70\"\n",
        )
        .unwrap();

        assert_eq!(
            read_rust_version(dir.path(), dir.path()),
            Some("1.75".to_string())
        );
    }
}
