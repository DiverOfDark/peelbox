//! Rust version detection and toolchain resolution.
//!
//! Version detection priority:
//! 1. `rust-toolchain.toml` — `[toolchain] channel = "1.75.0"`
//! 2. `rust-toolchain` — plain text channel string (e.g., `1.75.0`, `stable`, `nightly`)
//! 3. `Cargo.toml` — `rust-version = "1.70"` (MSRV)
//!
//! When a pinned version is not available in Wolfi, falls back to rustup installation.

use peelbox_core::output::schema::UniversalBuild;
use peelbox_wolfi::WolfiPackageIndex;
use std::path::Path;
use tracing::debug;

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
/// Expects `[toolchain]` section with a `channel` key (standard rust-toolchain.toml format).
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
    if trimmed.contains("[toolchain]") {
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

/// When a pinned Rust version (e.g., `rust-1.75`) is not available in Wolfi,
/// switch to installing it via rustup instead.
pub fn resolve_rust_toolchain(build: &mut UniversalBuild, wolfi: &WolfiPackageIndex) {
    if build.metadata.language != "Rust" {
        return;
    }

    // Find a rust-X.Y package that doesn't exist in Wolfi
    let pinned_version = build.build.packages.iter().find_map(|pkg| {
        pkg.strip_prefix("rust-").and_then(|version| {
            if version.chars().next().is_some_and(|c| c.is_ascii_digit()) && !wolfi.has_package(pkg)
            {
                Some(version.to_string())
            } else {
                None
            }
        })
    });

    let Some(version) = pinned_version else {
        return;
    };

    debug!(
        version = %version,
        "Pinned Rust version not in Wolfi, switching to rustup"
    );

    // Replace the unavailable rust-X.Y package with curl (for rustup download)
    for pkg in build.build.packages.iter_mut() {
        if pkg.starts_with("rust-") && !wolfi.has_package(pkg.as_str()) {
            *pkg = "curl".to_string();
            break;
        }
    }

    // Prepend rustup installation command
    build.build.commands.insert(
        0,
        format!(
            "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain {}",
            version
        ),
    );

    // Override CARGO_HOME to absolute path (matches rustup default) and add to PATH
    build
        .build
        .env
        .insert("CARGO_HOME".to_string(), "/root/.cargo".to_string());
    build.build.env.insert(
        "PATH".to_string(),
        "/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
    );

    // Update .cargo cache path to match the absolute CARGO_HOME
    for cache in build.build.cache.iter_mut() {
        if cache == ".cargo" {
            *cache = "/root/.cargo".to_string();
        }
    }

    // Copy the real cargo binary (not the rustup shim symlink) to runtime image
    // so it's available for apps that call cargo at runtime.
    // The rustup shim at /root/.cargo/bin/cargo is a symlink that requires the
    // full rustup toolchain metadata, so we resolve through to the actual binary.
    use peelbox_core::output::schema::CopySpec;

    // Copy the real cargo binary (not the rustup proxy shim) to runtime.
    // The rustup shim at /root/.cargo/bin/cargo is a multi-call binary that
    // needs the full rustup toolchain metadata. The actual standalone cargo
    // binary lives inside the toolchain directory under RUSTUP_HOME (~/.rustup).
    build.build.commands.push(
        "cp \"$(find /root/.rustup/toolchains -name cargo -path '*/bin/cargo' | head -1)\" /usr/local/bin/cargo".to_string(),
    );

    build.runtime.copy.push(CopySpec {
        from: "/usr/local/bin/cargo".to_string(),
        to: "/usr/local/bin/cargo".to_string(),
    });

    // Add runtime dependencies for the dynamically-linked cargo binary
    for dep in &["libgcc", "zlib", "libcurl-openssl4"] {
        let dep_str = dep.to_string();
        if !build.runtime.packages.contains(&dep_str) {
            build.runtime.packages.push(dep_str);
        }
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
    fn test_plain_toolchain_takes_precedence_over_cargo() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("rust-toolchain"), "1.80.0\n").unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\nrust-version = \"1.70\"\n",
        )
        .unwrap();

        assert_eq!(
            read_rust_version(dir.path(), dir.path()),
            Some("1.80".to_string())
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

    #[test]
    fn test_resolve_rust_toolchain_available_in_wolfi() {
        use std::collections::BTreeMap;

        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: peelbox_core::output::schema::BuildMetadata {
                project_name: Some("test".into()),
                language: "Rust".into(),
                build_system: "Cargo".into(),
                framework: None,
                reasoning: "test".into(),
            },
            build: peelbox_core::output::schema::BuildStage {
                packages: vec!["rust-1.92".into(), "build-base".into()],
                commands: vec!["cargo build --release".into()],
                env: BTreeMap::from([("CARGO_HOME".into(), ".cargo".into())]),
                cache: vec![".cargo".into(), "target".into()],
                build_image: None,
            },
            runtime: Default::default(),
        };

        resolve_rust_toolchain(&mut build, &wolfi);

        // rust-1.92 exists in Wolfi — no changes
        assert_eq!(build.build.packages[0], "rust-1.92");
        assert_eq!(build.build.commands.len(), 1);
        assert_eq!(build.build.env["CARGO_HOME"], ".cargo");
    }

    #[test]
    fn test_resolve_rust_toolchain_not_in_wolfi() {
        use std::collections::BTreeMap;

        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: peelbox_core::output::schema::BuildMetadata {
                project_name: Some("test".into()),
                language: "Rust".into(),
                build_system: "Cargo".into(),
                framework: None,
                reasoning: "test".into(),
            },
            build: peelbox_core::output::schema::BuildStage {
                packages: vec!["rust-1.50".into(), "build-base".into()],
                commands: vec!["cargo build --release".into()],
                env: BTreeMap::from([("CARGO_HOME".into(), ".cargo".into())]),
                cache: vec![".cargo".into(), "target".into()],
                build_image: None,
            },
            runtime: Default::default(),
        };

        resolve_rust_toolchain(&mut build, &wolfi);

        // rust-1.50 doesn't exist in Wolfi — should switch to rustup
        assert_eq!(build.build.packages[0], "curl");
        assert_eq!(build.build.commands.len(), 3);
        assert!(build.build.commands[0].contains("rustup"));
        assert!(build.build.commands[0].contains("1.50"));
        assert_eq!(build.build.commands[1], "cargo build --release");
        assert!(
            build.build.commands[2].contains("cp") && build.build.commands[2].contains("cargo")
        );
        assert_eq!(build.build.env["CARGO_HOME"], "/root/.cargo");
        assert!(build.build.env["PATH"].contains("/root/.cargo/bin"));
        assert_eq!(build.build.cache[0], "/root/.cargo");
        // Runtime should have cargo copy and lib dependencies
        assert!(build
            .runtime
            .copy
            .iter()
            .any(|c| c.to == "/usr/local/bin/cargo"));
        assert!(build.runtime.packages.contains(&"libgcc".to_string()));
    }

    #[test]
    fn test_resolve_rust_toolchain_skips_non_rust() {
        use std::collections::BTreeMap;

        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: peelbox_core::output::schema::BuildMetadata {
                project_name: Some("test".into()),
                language: "Python".into(),
                build_system: "pip".into(),
                framework: None,
                reasoning: "test".into(),
            },
            build: peelbox_core::output::schema::BuildStage {
                packages: vec!["python-3.11".into()],
                commands: vec!["pip install .".into()],
                env: BTreeMap::new(),
                cache: vec![],
                build_image: None,
            },
            runtime: Default::default(),
        };

        resolve_rust_toolchain(&mut build, &wolfi);

        // Non-Rust project — no changes
        assert_eq!(build.build.packages[0], "python-3.11");
        assert_eq!(build.build.commands.len(), 1);
    }
}
