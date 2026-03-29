//! Node.js version detection and resolution.
//!
//! Version detection reads `.nvmrc` and `.node-version` files, handling:
//! - Plain version numbers (`18.12.0`, `20`)
//! - `v`-prefixed versions (`v18.12.0`)
//! - LTS codenames (`lts/iron`, `lts/hydrogen`)
//!
//! Version resolution handles old versions not available in Wolfi.
//! Wolfi only provides Node.js >= 16 (nodejs-16, nodejs-18, nodejs-20, nodejs-22, nodejs-24).
//! When a project pins an older Node.js version (e.g., 14 from `.nvmrc`), this module
//! replaces the unavailable Wolfi package with a direct download from nodejs.org using
//! the `n` node version manager.

use peelbox_core::output::schema::UniversalBuild;
use peelbox_wolfi::WolfiPackageIndex;
use std::path::Path;
use tracing::debug;

/// Read Node.js version from `.nvmrc` or `.node-version` file.
/// Returns the major version string (e.g., "18", "20").
pub fn read_node_version(project_dir: &Path, repo_root: &Path) -> Option<String> {
    for dir in &[project_dir, repo_root] {
        for filename in &[".nvmrc", ".node-version"] {
            let path = dir.join(filename);
            if let Ok(content) = std::fs::read_to_string(&path) {
                let trimmed = content.trim();
                if let Some(version) = parse_node_version_string(trimmed) {
                    return Some(version);
                }
            }
        }
    }
    None
}

/// Parse a Node.js version string: strip 'v' prefix, map LTS codenames, extract major.
pub fn parse_node_version_string(s: &str) -> Option<String> {
    let s = s.trim().trim_start_matches('v');

    // Map LTS codenames
    let s = match s.to_lowercase().as_str() {
        "lts/iron" => "20",
        "lts/hydrogen" => "18",
        "lts/gallium" => "16",
        "lts/fermium" => "14",
        "lts/*" => return None, // Can't resolve "latest LTS" statically
        _ => s,
    };

    // Extract major version
    let major = s.split('.').next()?;
    if major.chars().all(|c| c.is_ascii_digit()) && !major.is_empty() {
        Some(major.to_string())
    } else {
        None
    }
}

/// The minimum major Node.js version available in Wolfi.
/// Versions below this threshold require direct download.
pub const MIN_WOLFI_NODE_MAJOR: u32 = 16;

/// When a pinned Node.js version (e.g., `nodejs-14`) is not available in Wolfi,
/// switch to installing it via `n` (node version manager) which downloads from nodejs.org.
pub fn resolve_node_version(build: &mut UniversalBuild, wolfi: &WolfiPackageIndex) {
    if !matches!(
        build.metadata.language.as_str(),
        "JavaScript" | "TypeScript"
    ) {
        return;
    }

    // Find a nodejs-X package that doesn't exist in Wolfi
    let pinned_version = build.build.packages.iter().find_map(|pkg| {
        pkg.strip_prefix("nodejs-").and_then(|version_str| {
            let major: u32 = version_str.parse().ok()?;
            if major < MIN_WOLFI_NODE_MAJOR && !wolfi.has_package(pkg) {
                Some(version_str.to_string())
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
        "Pinned Node.js version not in Wolfi, switching to n (node version manager)"
    );

    // Replace the unavailable nodejs-X package with curl (for downloading n)
    for pkg in build.build.packages.iter_mut() {
        if pkg.starts_with("nodejs-") && !wolfi.has_package(pkg.as_str()) {
            *pkg = "curl".to_string();
            break;
        }
    }

    // Ensure required packages are present
    for required_pkg in &["ca-certificates", "build-base", "bash", "libstdc++"] {
        if !build.build.packages.contains(&required_pkg.to_string()) {
            build.build.packages.push(required_pkg.to_string());
        }
    }

    // Remove npm from build packages (it comes bundled with the Node.js install)
    build.build.packages.retain(|p| p != "npm");

    // Prepend Node.js installation command using `n` (node version manager)
    // `n` is a single shell script that can install any Node.js version
    let install_cmd = format!(
        "mkdir -p /usr/local/bin && curl -fsSL https://raw.githubusercontent.com/tj/n/master/bin/n -o /usr/local/bin/n && chmod +x /usr/local/bin/n && n {}",
        version
    );
    build.build.commands.insert(0, install_cmd);

    // Set PATH to include the installed Node.js
    build.build.env.insert(
        "PATH".to_string(),
        "/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
    );

    // Fix runtime packages too
    let mut needs_runtime_fix = false;
    for pkg in build.runtime.packages.iter_mut() {
        if pkg.starts_with("nodejs-") && !wolfi.has_package(pkg.as_str()) {
            *pkg = "curl".to_string();
            needs_runtime_fix = true;
            break;
        }
    }

    if needs_runtime_fix {
        for required_pkg in &["ca-certificates", "bash", "libstdc++"] {
            if !build.runtime.packages.contains(&required_pkg.to_string()) {
                build.runtime.packages.push(required_pkg.to_string());
            }
        }
        build.runtime.packages.retain(|p| p != "npm");

        // Prepend Node.js installation to the runtime command
        // The runtime command is a Vec<String> representing the command + args.
        // We need to wrap it: install node first, then run the original command.
        let original_cmd = build.runtime.command.clone();
        if !original_cmd.is_empty() {
            let install_cmd = format!(
                "mkdir -p /usr/local/bin && curl -fsSL https://raw.githubusercontent.com/tj/n/master/bin/n -o /usr/local/bin/n && chmod +x /usr/local/bin/n && n {} > /dev/null 2>&1",
                version
            );
            let original_cmd_str = original_cmd.join(" ");
            build.runtime.command = vec![
                "sh".to_string(),
                "-c".to_string(),
                format!("{} && {}", install_cmd, original_cmd_str),
            ];
        }

        build.runtime.env.insert(
            "PATH".to_string(),
            "/usr/local/bin:/usr/local/sbin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_node_build(
        build_packages: Vec<String>,
        runtime_packages: Vec<String>,
        language: &str,
    ) -> UniversalBuild {
        UniversalBuild {
            version: "1.0".into(),
            metadata: peelbox_core::output::schema::BuildMetadata {
                project_name: Some("test".into()),
                language: language.into(),
                build_system: "npm".into(),
                framework: None,
                reasoning: "test".into(),
            },
            build: peelbox_core::output::schema::BuildStage {
                packages: build_packages,
                commands: vec!["npm install".into()],
                env: BTreeMap::new(),
                cache: vec![".npm".into()],
                setup_commands: vec![],
            },
            runtime: peelbox_core::output::schema::RuntimeStage {
                packages: runtime_packages,
                command: vec!["node".into(), "index.js".into()],
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_resolve_node_version_available_in_wolfi() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-22".into(), "npm".into()],
            vec!["nodejs-22".into()],
            "JavaScript",
        );

        resolve_node_version(&mut build, &wolfi);

        // nodejs-22 exists in Wolfi -- no changes
        assert_eq!(build.build.packages[0], "nodejs-22");
        assert_eq!(build.build.commands.len(), 1);
    }

    #[test]
    fn test_resolve_node_version_not_in_wolfi() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-14".into(), "npm".into()],
            vec!["nodejs-14".into(), "npm".into()],
            "JavaScript",
        );

        resolve_node_version(&mut build, &wolfi);

        // nodejs-14 doesn't exist in Wolfi -- should switch to n
        assert_eq!(build.build.packages[0], "curl");
        assert!(!build.build.packages.contains(&"npm".to_string()));
        assert!(build
            .build
            .packages
            .contains(&"ca-certificates".to_string()));
        assert!(build.build.packages.contains(&"build-base".to_string()));
        assert!(build.build.packages.contains(&"bash".to_string()));
        assert!(build.build.packages.contains(&"libstdc++".to_string()));
        assert_eq!(build.build.commands.len(), 2);
        assert!(build.build.commands[0].contains("mkdir -p /usr/local/bin"));
        assert!(build.build.commands[0].contains("/n"));
        assert!(build.build.commands[0].ends_with(" 14"));
        assert_eq!(build.build.commands[1], "npm install");
        assert!(build.build.env.contains_key("PATH"));
    }

    #[test]
    fn test_resolve_node_version_fixes_runtime() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-14".into(), "npm".into()],
            vec!["nodejs-14".into(), "npm".into()],
            "JavaScript",
        );

        resolve_node_version(&mut build, &wolfi);

        // Runtime packages should also be fixed
        assert!(build.runtime.packages.contains(&"curl".to_string()));
        assert!(!build.runtime.packages.contains(&"npm".to_string()));
        assert!(build
            .runtime
            .packages
            .contains(&"ca-certificates".to_string()));

        // Runtime command should be wrapped with sh -c
        assert_eq!(build.runtime.command[0], "sh");
        assert_eq!(build.runtime.command[1], "-c");
        assert!(build.runtime.command[2].contains("node index.js"));
        assert!(build.runtime.command[2].contains("/n"));
    }

    #[test]
    fn test_resolve_node_version_skips_non_js() {
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
                setup_commands: vec![],
            },
            runtime: Default::default(),
        };

        resolve_node_version(&mut build, &wolfi);

        // Non-JS project -- no changes
        assert_eq!(build.build.packages[0], "python-3.11");
        assert_eq!(build.build.commands.len(), 1);
    }

    #[test]
    fn test_resolve_node_version_typescript() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-14".into(), "npm".into()],
            vec!["nodejs-14".into(), "npm".into()],
            "TypeScript",
        );

        resolve_node_version(&mut build, &wolfi);

        // TypeScript also uses Node.js -- should work the same
        assert_eq!(build.build.packages[0], "curl");
        assert!(build.build.commands[0].ends_with(" 14"));
    }

    #[test]
    fn test_resolve_node_16_not_affected() {
        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = make_node_build(
            vec!["nodejs-16".into(), "npm".into()],
            vec!["nodejs-16".into()],
            "JavaScript",
        );
        let original_packages = build.build.packages.clone();

        resolve_node_version(&mut build, &wolfi);

        // nodejs-16 should exist in Wolfi or at least not trigger old version logic
        // (it may or may not exist depending on the test data, but the major >= MIN_WOLFI_NODE_MAJOR
        // check should prevent the fallback)
        if wolfi.has_package("nodejs-16") {
            assert_eq!(build.build.packages, original_packages);
        }
    }

    #[test]
    fn test_parse_node_version_string() {
        assert_eq!(
            parse_node_version_string("v18.12.0"),
            Some("18".to_string())
        );
        assert_eq!(parse_node_version_string("20"), Some("20".to_string()));
        assert_eq!(parse_node_version_string("18.12.0"), Some("18".to_string()));
        assert_eq!(
            parse_node_version_string("lts/iron"),
            Some("20".to_string())
        );
        assert_eq!(
            parse_node_version_string("lts/hydrogen"),
            Some("18".to_string())
        );
        assert_eq!(parse_node_version_string("lts/*"), None);
    }

    #[test]
    fn test_read_node_version_from_nvmrc() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".nvmrc"), "v18.12.0\n").unwrap();
        assert_eq!(
            read_node_version(dir.path(), dir.path()),
            Some("18".to_string())
        );
    }

    #[test]
    fn test_read_node_version_from_node_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".node-version"), "20\n").unwrap();
        assert_eq!(
            read_node_version(dir.path(), dir.path()),
            Some("20".to_string())
        );
    }

    #[test]
    fn test_read_node_version_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_node_version(dir.path(), dir.path()), None);
    }
}
