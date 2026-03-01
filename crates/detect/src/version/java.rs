//! Shared Java/Kotlin version detection
//!
//! Consolidates Java version parsing from pom.xml, build.gradle(.kts),
//! and .java-version files.
//!
//! Java 1.x version normalization: Old-style Java versions like "1.8" are
//! normalized to their modern equivalent ("8"). This naming convention was
//! used through Java 8 and deprecated starting with Java 9.
//!
//! Adoptium fallback: When the detected Java version is older than the minimum
//! available in Wolfi (currently Java 11), the pipeline falls back to downloading
//! a JDK from Eclipse Adoptium (Temurin) instead of using Wolfi packages.

use peelbox_core::output::schema::{CopySpec, UniversalBuild};
use peelbox_wolfi::WolfiPackageIndex;
use regex::Regex;
use tracing::debug;

/// Minimum Java version available as a Wolfi `openjdk-*` package.
/// Versions below this require Adoptium/Temurin download instead.
pub const MIN_WOLFI_JAVA_VERSION: u32 = 11;

/// Normalize old-style Java 1.x version strings to their modern equivalent.
///
/// Java versions 1.0 through 1.8 used a `1.x` naming convention.
/// Starting with Java 9, the major version number is used directly.
///
/// Examples:
/// - "1.8" -> "8"
/// - "1.7" -> "7"
/// - "1.5" -> "5"
/// - "11" -> "11" (unchanged)
/// - "17" -> "17" (unchanged)
/// - "1.8.0_292" -> "8" (strips everything after major)
pub fn normalize_java_version(version: &str) -> String {
    let trimmed = version.trim();

    // Handle 1.x style versions (Java 1.0 through 1.8)
    if let Some(rest) = trimmed.strip_prefix("1.") {
        // Extract just the minor version number (the actual Java version)
        let minor: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !minor.is_empty() {
            if let Ok(v) = minor.parse::<u32>() {
                if v <= 8 {
                    return minor;
                }
            }
        }
    }

    // For modern versions (9+), extract just the major version number
    let major: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !major.is_empty() {
        return major;
    }

    trimmed.to_string()
}

/// Detect Java version from manifest content (pom.xml, build.gradle, .java-version).
/// Returns the normalized version number (e.g., "17", "21", "11", "8").
///
/// Old-style `1.x` versions are automatically normalized: `1.8` -> `8`.
pub fn detect_java_version(manifest_content: &str) -> Option<String> {
    // Try pom.xml patterns first
    if let Some(ver) = parse_pom_version(manifest_content) {
        return Some(normalize_java_version(&ver));
    }

    // Try build.gradle(.kts) patterns
    if let Some(ver) = parse_gradle_version(manifest_content) {
        return Some(normalize_java_version(&ver));
    }

    // Try .java-version file (plain text version number)
    parse_java_version_file(manifest_content).map(|v| normalize_java_version(&v))
}

/// Parse Java version from pom.xml content using XML parsing.
/// Looks for `<maven.compiler.source>`, `<java.version>`, `<maven.compiler.release>`.
/// Returns raw version number (e.g., "17", "1.8").
pub fn parse_pom_version(content: &str) -> Option<String> {
    // Quick check: only try XML parsing if it looks like XML
    if !content.contains('<') {
        return None;
    }

    if let Ok(doc) = roxmltree::Document::parse(content) {
        for node in doc.descendants() {
            if node.has_tag_name("maven.compiler.source")
                || node.has_tag_name("java.version")
                || node.has_tag_name("maven.compiler.release")
            {
                if let Some(version) = node.text() {
                    return Some(version.trim().to_string());
                }
            }
        }
    }

    // Fallback to regex for cases where XML parsing fails
    // Match both modern (17) and legacy (1.8) version formats
    // <maven.compiler.source>17</maven.compiler.source> or <maven.compiler.source>1.8</maven.compiler.source>
    if let Some(caps) =
        Regex::new(r"<maven\.compiler\.source>([\d.]+)</maven\.compiler\.source>")
            .ok()
            .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }
    // <java.version>17</java.version> or <java.version>1.8</java.version>
    if let Some(caps) = Regex::new(r"<java\.version>([\d.]+)</java\.version>")
        .ok()
        .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }
    // <release>17</release>
    if let Some(caps) = Regex::new(r"<release>([\d.]+)</release>")
        .ok()
        .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    None
}

/// Parse Java version from build.gradle or build.gradle.kts content.
/// Looks for `sourceCompatibility`, `targetCompatibility`, `languageVersion`.
/// Returns raw version number (e.g., "17").
pub fn parse_gradle_version(content: &str) -> Option<String> {
    // sourceCompatibility = JavaVersion.VERSION_17 or "17" or JavaVersion.VERSION_1_8
    if let Some(caps) =
        Regex::new(r#"sourceCompatibility\s*=\s*(?:JavaVersion\.VERSION_)?["']?([\d._]+)"#)
            .ok()
            .and_then(|re| re.captures(content))
    {
        let raw = caps.get(1).map(|m| m.as_str())?;
        // Handle VERSION_1_8 style (underscores become dots)
        let cleaned = raw.replace('_', ".");
        return Some(cleaned);
    }

    // languageVersion.set(JavaLanguageVersion.of(21)) or languageVersion = JavaLanguageVersion.of(21)
    if let Some(caps) =
        Regex::new(r"languageVersion(?:\.set)?(?:\s*=\s*|\s+|\()JavaLanguageVersion\.of\((\d+)\)")
            .ok()
            .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    // targetCompatibility as fallback
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.contains("targetCompatibility") {
            if let Some(version) = trimmed.split(['=', '(', ')', ' ']).find(|s| {
                let s = s.trim();
                !s.is_empty() && (s.chars().all(|c| c.is_ascii_digit()) || s.contains("VERSION_"))
            }) {
                let version_num = version
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .replace("JavaVersion.VERSION_", "")
                    .replace('_', ".");

                let version_final = if version_num.starts_with("1.") && version_num.len() > 2 {
                    version_num.get(2..).unwrap_or(&version_num).to_string()
                } else {
                    version_num
                };

                return Some(version_final);
            }
        }
    }

    None
}

/// Parse version from a .java-version file (plain text version number).
fn parse_java_version_file(content: &str) -> Option<String> {
    if content.contains('<') || content.contains('{') {
        return None;
    }
    let trimmed = content.trim();
    if Regex::new(r"^\d+(\.\d+)*$").ok()?.is_match(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Returns the Java version formatted for Wolfi package names (e.g., "openjdk-17").
pub fn detect_java_version_wolfi(manifest_content: &str) -> Option<String> {
    detect_java_version(manifest_content).map(|v| format!("openjdk-{}", v))
}

/// Returns true if the given Java major version is too old for Wolfi packages.
pub fn is_legacy_java_version(version: &str) -> bool {
    version
        .parse::<u32>()
        .map(|v| v < MIN_WOLFI_JAVA_VERSION)
        .unwrap_or(false)
}

/// When a Java version is too old for Wolfi (< 11), replace openjdk packages
/// with an Adoptium/Temurin JDK download and set up JAVA_HOME/PATH.
///
/// This follows the same pattern as `resolve_rust_toolchain` for Rust versions
/// not available in Wolfi.
///
/// Build stage: Removes openjdk packages, adds curl/ca-certificates, prepends
/// JDK download command, updates JAVA_HOME and PATH.
///
/// Runtime stage: Removes openjdk packages, adds a CopySpec to copy the JDK
/// from the build stage into the runtime image, updates JAVA_HOME and PATH.
pub fn resolve_java_toolchain(build: &mut UniversalBuild, wolfi: &WolfiPackageIndex) {
    let lang = &build.metadata.language;
    if lang != "Java" && lang != "Kotlin" && lang != "Scala" {
        return;
    }

    // Find an openjdk-N package where N < MIN_WOLFI_JAVA_VERSION
    let legacy_version = find_legacy_openjdk_version(&build.build.packages, wolfi)
        .or_else(|| find_legacy_openjdk_version(&build.runtime.packages, wolfi));

    let version = match legacy_version {
        Some(v) => v,
        None => return,
    };

    debug!(
        version = %version,
        "Java version too old for Wolfi (minimum {}), switching to Adoptium Temurin",
        MIN_WOLFI_JAVA_VERSION
    );

    let java_home = format!("/usr/lib/jvm/java-{}", version);
    let java_home_bin = format!("{}/bin", java_home);
    let install_cmd = format!(
        "curl -fsSL https://api.adoptium.net/v3/binary/latest/{version}/ga/linux/x64/jdk/hotspot/normal/eclipse \
         -o /tmp/jdk.tar.gz && \
         mkdir -p {java_home} && \
         tar -xzf /tmp/jdk.tar.gz -C {java_home} --strip-components=1 && \
         rm /tmp/jdk.tar.gz",
        version = version,
        java_home = java_home,
    );

    // Replace openjdk-N packages in build stage with curl + ca-certificates
    replace_legacy_openjdk_packages(&mut build.build.packages, &version);
    ensure_package(&mut build.build.packages, "curl");
    ensure_package(&mut build.build.packages, "ca-certificates");

    // Prepend the JDK installation command
    build.build.commands.insert(0, install_cmd);

    // Update build env
    build
        .build
        .env
        .insert("JAVA_HOME".to_string(), java_home.clone());
    let path_value = format!(
        "{}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        java_home_bin
    );
    build.build.env.insert("PATH".to_string(), path_value);

    // Replace openjdk-N-jre packages in runtime stage
    replace_legacy_openjdk_packages(&mut build.runtime.packages, &version);

    // Add CopySpec to copy the JDK from build stage to runtime
    // The JDK was installed at /usr/lib/jvm/java-{version}/ during build
    build.runtime.copy.push(CopySpec {
        from: format!("{}/", java_home),
        to: format!("{}/", java_home),
    });

    // Update runtime env
    build
        .runtime
        .env
        .insert("JAVA_HOME".to_string(), java_home.clone());
    build.runtime.env.insert(
        "PATH".to_string(),
        format!(
            "{}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            java_home_bin
        ),
    );
}

/// Find a legacy (pre-Wolfi) openjdk version in a package list.
/// Returns the version number (e.g., "8") if found.
///
/// A version is considered legacy if it is below `MIN_WOLFI_JAVA_VERSION` (11).
/// These versions are never available in Wolfi, so we check the version number
/// directly rather than querying the package index.
fn find_legacy_openjdk_version(packages: &[String], _wolfi: &WolfiPackageIndex) -> Option<String> {
    for pkg in packages {
        let version_str = if let Some(v) = pkg.strip_prefix("openjdk-") {
            // Handle both openjdk-8 and openjdk-8-jre
            v.split('-').next().unwrap_or(v)
        } else {
            continue;
        };

        if let Ok(ver) = version_str.parse::<u32>() {
            if ver < MIN_WOLFI_JAVA_VERSION {
                return Some(version_str.to_string());
            }
        }
    }
    None
}

/// Remove legacy openjdk packages from a package list.
fn replace_legacy_openjdk_packages(packages: &mut Vec<String>, version: &str) {
    packages.retain(|pkg| {
        // Remove openjdk-{version} and openjdk-{version}-jre
        if let Some(v) = pkg.strip_prefix("openjdk-") {
            let pkg_version = v.split('-').next().unwrap_or(v);
            pkg_version != version
        } else {
            true
        }
    });
}

/// Ensure a package is in the list (add if missing).
fn ensure_package(packages: &mut Vec<String>, pkg: &str) {
    if !packages.iter().any(|p| p == pkg) {
        packages.push(pkg.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_java_version tests ──────────────────────────────────────

    #[test]
    fn test_normalize_1_8() {
        assert_eq!(normalize_java_version("1.8"), "8");
    }

    #[test]
    fn test_normalize_1_7() {
        assert_eq!(normalize_java_version("1.7"), "7");
    }

    #[test]
    fn test_normalize_1_5() {
        assert_eq!(normalize_java_version("1.5"), "5");
    }

    #[test]
    fn test_normalize_1_8_0_292() {
        assert_eq!(normalize_java_version("1.8.0_292"), "8");
    }

    #[test]
    fn test_normalize_modern_11() {
        assert_eq!(normalize_java_version("11"), "11");
    }

    #[test]
    fn test_normalize_modern_17() {
        assert_eq!(normalize_java_version("17"), "17");
    }

    #[test]
    fn test_normalize_modern_21() {
        assert_eq!(normalize_java_version("21"), "21");
    }

    // ── detect_java_version tests ─────────────────────────────────────────

    #[test]
    fn test_pom_maven_compiler_source() {
        let content = r#"<project><properties><maven.compiler.source>17</maven.compiler.source></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("17".to_string()));
    }

    #[test]
    fn test_pom_java_version() {
        let content =
            r#"<project><properties><java.version>21</java.version></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("21".to_string()));
    }

    #[test]
    fn test_pom_maven_compiler_release() {
        let content = r#"<project><properties><maven.compiler.release>11</maven.compiler.release></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("11".to_string()));
    }

    #[test]
    fn test_pom_legacy_java_8() {
        let content = r#"<project><properties><maven.compiler.source>1.8</maven.compiler.source></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("8".to_string()));
    }

    #[test]
    fn test_pom_legacy_java_7() {
        let content = r#"<project><properties><java.version>1.7</java.version></properties></project>"#;
        assert_eq!(detect_java_version(content), Some("7".to_string()));
    }

    #[test]
    fn test_gradle_source_compat() {
        let content = r#"sourceCompatibility = "17""#;
        assert_eq!(detect_java_version(content), Some("17".to_string()));
    }

    #[test]
    fn test_gradle_toolchain() {
        let content = r#"java { toolchain { languageVersion.set(JavaLanguageVersion.of(21)) } }"#;
        assert_eq!(detect_java_version(content), Some("21".to_string()));
    }

    #[test]
    fn test_gradle_legacy_version_1_8() {
        let content = r#"sourceCompatibility = JavaVersion.VERSION_1_8"#;
        assert_eq!(detect_java_version(content), Some("8".to_string()));
    }

    #[test]
    fn test_java_version_file() {
        assert_eq!(detect_java_version("17"), Some("17".to_string()));
        assert_eq!(detect_java_version("17.0"), Some("17".to_string()));
    }

    #[test]
    fn test_java_version_file_legacy() {
        assert_eq!(detect_java_version("1.8"), Some("8".to_string()));
        assert_eq!(detect_java_version("1.8.0"), Some("8".to_string()));
    }

    #[test]
    fn test_no_version() {
        assert_eq!(detect_java_version("<project></project>"), None);
    }

    #[test]
    fn test_wolfi_format() {
        let content =
            r#"<project><properties><java.version>17</java.version></properties></project>"#;
        assert_eq!(
            detect_java_version_wolfi(content),
            Some("openjdk-17".to_string())
        );
    }

    #[test]
    fn test_wolfi_format_legacy() {
        let content =
            r#"<project><properties><java.version>1.8</java.version></properties></project>"#;
        assert_eq!(
            detect_java_version_wolfi(content),
            Some("openjdk-8".to_string())
        );
    }

    #[test]
    fn test_pom_version_standalone() {
        let content = r#"<project><properties><maven.compiler.source>21</maven.compiler.source></properties></project>"#;
        assert_eq!(parse_pom_version(content), Some("21".to_string()));
    }

    #[test]
    fn test_pom_version_standalone_legacy() {
        let content = r#"<project><properties><maven.compiler.source>1.8</maven.compiler.source></properties></project>"#;
        assert_eq!(parse_pom_version(content), Some("1.8".to_string()));
    }

    #[test]
    fn test_gradle_version_standalone() {
        let content = r#"sourceCompatibility = "17""#;
        assert_eq!(parse_gradle_version(content), Some("17".to_string()));
    }

    #[test]
    fn test_gradle_version_version_prefix() {
        let content = r#"sourceCompatibility = JavaVersion.VERSION_17"#;
        assert_eq!(parse_gradle_version(content), Some("17".to_string()));
    }

    // ── is_legacy_java_version tests ──────────────────────────────────────

    #[test]
    fn test_is_legacy_java_8() {
        assert!(is_legacy_java_version("8"));
    }

    #[test]
    fn test_is_legacy_java_7() {
        assert!(is_legacy_java_version("7"));
    }

    #[test]
    fn test_is_not_legacy_java_11() {
        assert!(!is_legacy_java_version("11"));
    }

    #[test]
    fn test_is_not_legacy_java_17() {
        assert!(!is_legacy_java_version("17"));
    }

    // ── resolve_java_toolchain tests ──────────────────────────────────────

    #[test]
    fn test_resolve_java_toolchain_modern_version() {
        use std::collections::BTreeMap;

        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: peelbox_core::output::schema::BuildMetadata {
                project_name: Some("test".into()),
                language: "Java".into(),
                build_system: "Maven".into(),
                framework: None,
                reasoning: "test".into(),
            },
            build: peelbox_core::output::schema::BuildStage {
                packages: vec!["openjdk-17".into(), "maven".into()],
                commands: vec!["mvn package -DskipTests".into()],
                env: BTreeMap::new(),
                cache: vec![],
            },
            runtime: peelbox_core::output::schema::RuntimeStage {
                packages: vec!["openjdk-17-jre".into()],
                ..Default::default()
            },
        };

        resolve_java_toolchain(&mut build, &wolfi);

        // openjdk-17 exists in Wolfi — no changes
        assert!(build
            .build
            .packages
            .iter()
            .any(|p| p.starts_with("openjdk-17")));
        assert_eq!(build.build.commands.len(), 1);
    }

    #[test]
    fn test_resolve_java_toolchain_legacy_version() {
        use std::collections::BTreeMap;

        let wolfi = WolfiPackageIndex::for_tests();
        let mut build = UniversalBuild {
            version: "1.0".into(),
            metadata: peelbox_core::output::schema::BuildMetadata {
                project_name: Some("test".into()),
                language: "Java".into(),
                build_system: "Maven".into(),
                framework: None,
                reasoning: "test".into(),
            },
            build: peelbox_core::output::schema::BuildStage {
                packages: vec![
                    "openjdk-8".into(),
                    "maven".into(),
                    "ca-certificates".into(),
                ],
                commands: vec!["mvn package -DskipTests".into()],
                env: BTreeMap::from([(
                    "JAVA_HOME".into(),
                    "/usr/lib/jvm/java-8-openjdk".into(),
                )]),
                cache: vec!["/root/.m2/repository".into()],
            },
            runtime: peelbox_core::output::schema::RuntimeStage {
                packages: vec!["openjdk-8-jre".into(), "ca-certificates".into()],
                env: BTreeMap::from([
                    ("JAVA_HOME".into(), "/usr/lib/jvm/java-8-openjdk".into()),
                    (
                        "PATH".into(),
                        "/usr/lib/jvm/java-8-openjdk/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into()
                    ),
                ]),
                ..Default::default()
            },
        };

        resolve_java_toolchain(&mut build, &wolfi);

        // openjdk-8 doesn't exist in Wolfi — should switch to Adoptium
        assert!(
            !build
                .build
                .packages
                .iter()
                .any(|p| p.starts_with("openjdk")),
            "openjdk packages should be removed: {:?}",
            build.build.packages
        );
        assert!(
            build.build.packages.contains(&"curl".to_string()),
            "curl should be added for Adoptium download"
        );
        assert!(
            build.build.commands[0].contains("adoptium.net"),
            "First command should be Adoptium download"
        );
        assert!(
            build.build.commands[0].contains("/8/"),
            "Download URL should reference Java 8"
        );
        assert_eq!(build.build.env["JAVA_HOME"], "/usr/lib/jvm/java-8");

        // Runtime should also be updated
        assert!(
            !build
                .runtime
                .packages
                .iter()
                .any(|p| p.starts_with("openjdk")),
            "runtime openjdk packages should be removed: {:?}",
            build.runtime.packages
        );
        assert_eq!(build.runtime.env["JAVA_HOME"], "/usr/lib/jvm/java-8");

        // Should have a copy spec for the JDK
        assert!(
            build
                .runtime
                .copy
                .iter()
                .any(|c| c.from.contains("java-8")),
            "Should copy JDK from build to runtime: {:?}",
            build.runtime.copy
        );
    }

    #[test]
    fn test_resolve_java_toolchain_skips_non_java() {
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
            },
            runtime: Default::default(),
        };

        resolve_java_toolchain(&mut build, &wolfi);

        // Non-Java project — no changes
        assert_eq!(build.build.packages[0], "python-3.11");
        assert_eq!(build.build.commands.len(), 1);
    }
}
