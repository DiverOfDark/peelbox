//! Shared Java/Kotlin version detection
//!
//! Consolidates Java version parsing from pom.xml, build.gradle(.kts),
//! and .java-version files.

use regex::Regex;

/// Detect Java version from manifest content (pom.xml, build.gradle, .java-version).
/// Returns the raw version number (e.g., "17", "21", "11").
pub fn detect_java_version(manifest_content: &str) -> Option<String> {
    // Try pom.xml patterns first
    if let Some(ver) = parse_pom_version(manifest_content) {
        return Some(ver);
    }

    // Try build.gradle(.kts) patterns
    if let Some(ver) = parse_gradle_version(manifest_content) {
        return Some(ver);
    }

    // Try .java-version file (plain text version number)
    parse_java_version_file(manifest_content)
}

/// Parse Java version from pom.xml content using XML parsing.
/// Looks for `<maven.compiler.source>`, `<java.version>`, `<maven.compiler.release>`.
/// Returns raw version number (e.g., "17").
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
    // <maven.compiler.source>17</maven.compiler.source>
    if let Some(caps) =
        Regex::new(r"<maven\.compiler\.source>(\d+)</maven\.compiler\.source>")
            .ok()
            .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }
    // <java.version>17</java.version>
    if let Some(caps) = Regex::new(r"<java\.version>(\d+)</java\.version>")
        .ok()
        .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }
    // <release>17</release>
    if let Some(caps) = Regex::new(r"<release>(\d+)</release>")
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
    // sourceCompatibility = JavaVersion.VERSION_17 or "17"
    if let Some(caps) =
        Regex::new(r#"sourceCompatibility\s*=\s*(?:JavaVersion\.VERSION_)?["']?(\d+)"#)
            .ok()
            .and_then(|re| re.captures(content))
    {
        return caps.get(1).map(|m| m.as_str().to_string());
    }

    // languageVersion.set(JavaLanguageVersion.of(21)) or languageVersion = JavaLanguageVersion.of(21)
    if let Some(caps) = Regex::new(
        r"languageVersion(?:\.set)?(?:\s*=\s*|\s+|\()JavaLanguageVersion\.of\((\d+)\)",
    )
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
                !s.is_empty()
                    && (s.chars().all(|c| c.is_ascii_digit()) || s.contains("VERSION_"))
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
    if Regex::new(r"^\d+(\.\d+)?$").ok()?.is_match(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Returns the Java version formatted for Wolfi package names (e.g., "openjdk-17").
pub fn detect_java_version_wolfi(manifest_content: &str) -> Option<String> {
    detect_java_version(manifest_content).map(|v| format!("openjdk-{}", v))
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_java_version_file() {
        assert_eq!(detect_java_version("17"), Some("17".to_string()));
        assert_eq!(detect_java_version("17.0"), Some("17.0".to_string()));
    }

    #[test]
    fn test_no_version() {
        assert_eq!(detect_java_version("<project></project>"), None);
    }

    #[test]
    fn test_wolfi_format() {
        let content = r#"<project><properties><java.version>17</java.version></properties></project>"#;
        assert_eq!(
            detect_java_version_wolfi(content),
            Some("openjdk-17".to_string())
        );
    }

    #[test]
    fn test_pom_version_standalone() {
        let content = r#"<project><properties><maven.compiler.source>21</maven.compiler.source></properties></project>"#;
        assert_eq!(parse_pom_version(content), Some("21".to_string()));
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
}
