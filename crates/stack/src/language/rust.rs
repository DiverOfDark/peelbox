//! Rust language definition

#[cfg(test)]
use super::DetectionMethod;
use super::{
    parsers::{DependencyParser, TomlDependencyParser},
    DependencyInfo, DetectionResult, LanguageDefinition,
};

/// Rust language definition
pub struct RustLanguage;

impl LanguageDefinition for RustLanguage {
    fn id(&self) -> crate::LanguageId {
        crate::LanguageId::Rust
    }

    fn extensions(&self) -> Vec<String> {
        vec!["rs".to_string()]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        if manifest_name != "Cargo.toml" {
            return None;
        }

        let mut confidence = 0.9;

        if let Some(content) = manifest_content {
            if content.contains("[package]") || content.contains("[workspace]") {
                confidence = 1.0;
            }
        }

        Some(DetectionResult {
            build_system: crate::BuildSystemId::Cargo,
            confidence,
        })
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["cargo".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec!["target".to_string(), ".cargo".to_string()]
    }

    fn detect_version(&self, manifest_content: Option<&str>) -> Option<String> {
        let content = manifest_content?;
        // Check rust-toolchain.toml or rust-toolchain file format
        // channel = "1.75" or just "1.75"
        if content.contains("channel") {
            if let Some(start) = content.find("channel") {
                let after = &content[start..];
                if let Some(quote_start) = after.find('"') {
                    let after_quote = &after[quote_start + 1..];
                    if let Some(quote_end) = after_quote.find('"') {
                        return Some(after_quote[..quote_end].to_string());
                    }
                }
            }
        }
        // Simple version string
        let trimmed = content.trim();
        if trimmed.starts_with("1.") && trimmed.len() < 10 {
            return Some(trimmed.to_string());
        }
        None
    }

    fn is_workspace_root(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "Cargo.toml" {
            return false;
        }

        if let Some(content) = manifest_content {
            content.contains("[workspace]")
        } else {
            false
        }
    }

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        TomlDependencyParser {
            dependencies_keys: &["dependencies", "dev-dependencies", "build-dependencies"],
            workspace_members_key: Some("members"),
        }
        .parse(manifest_content, all_internal_paths)
    }

    fn env_var_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r#"std::env::var\(["']([A-Z_][A-Z0-9_]*)["']"#.to_string(),
                "std::env".to_string(),
            ),
            (
                r#"env::var\(["']([A-Z_][A-Z0-9_]*)["']"#.to_string(),
                "env::var".to_string(),
            ),
        ]
    }

    fn port_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r"\.bind\([^,)]*:(\d{4,5})".to_string(),
                "bind()".to_string(),
            ),
            (
                r#"addr\s*=\s*"[^:]*:(\d{4,5})""#.to_string(),
                "addr config".to_string(),
            ),
        ]
    }

    fn health_check_patterns(&self) -> Vec<(String, String)> {
        vec![
            (
                r#"\.route\(['"]([/\w\-]*health[/\w\-]*)['"]"#.to_string(),
                "axum/actix".to_string(),
            ),
            (
                r#"\.get\(['"]([/\w\-]*health[/\w\-]*)['"]"#.to_string(),
                "rocket/warp".to_string(),
            ),
        ]
    }

    fn is_main_file(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        file_path: &std::path::Path,
    ) -> bool {
        // Check if filename matches known entry points
        if let Some(filename) = file_path.file_name().and_then(|f| f.to_str()) {
            if filename == "main.rs" || filename == "lib.rs" {
                return true;
            }
        }

        // For other .rs files in bin/, check for main function
        let path_str = file_path.to_string_lossy();
        if path_str.contains("/bin/") && path_str.ends_with(".rs") {
            if let Ok(content) = fs.read_to_string(file_path) {
                use regex::Regex;
                let main_re = Regex::new(r"fn\s+main\s*\(").expect("valid regex");
                return main_re.is_match(&content);
            }
        }

        false
    }

    fn runtime_name(&self) -> Option<String> {
        Some("rust".to_string())
    }

    fn default_port(&self) -> Option<u16> {
        Some(8080)
    }

    fn default_entrypoint(&self, build_system: &str) -> Option<String> {
        match build_system {
            "cargo" => Some("./target/release/{project_name}".to_string()),
            _ => None,
        }
    }

    fn parse_entrypoint_from_manifest(&self, manifest_content: &str) -> Option<String> {
        let parsed: toml::Value = toml::from_str(manifest_content).ok()?;
        let package_name = parsed
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())?;
        Some(format!("./target/release/{}", package_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extensions() {
        let lang = RustLanguage;
        assert_eq!(lang.extensions(), vec!["rs".to_string()]);
    }

    #[test]
    fn test_detect_cargo_toml() {
        let lang = RustLanguage;
        let result = lang.detect("Cargo.toml", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, crate::BuildSystemId::Cargo);
        assert_eq!(r.confidence, 0.9);
    }

    #[test]
    fn test_detect_with_content() {
        let lang = RustLanguage;
        let content = r#"
[package]
name = "myapp"
version = "0.1.0"
"#;
        let result = lang.detect("Cargo.toml", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_workspace() {
        let lang = RustLanguage;
        let content = r#"
[workspace]
members = ["crate1", "crate2"]
"#;
        let result = lang.detect("Cargo.toml", Some(content));
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn test_detect_non_manifest() {
        let lang = RustLanguage;
        let result = lang.detect("package.json", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_compatible_build_systems() {
        let lang = RustLanguage;
        assert_eq!(lang.compatible_build_systems(), vec!["cargo".to_string()]);
    }

    #[test]
    fn test_is_workspace_root_true() {
        let lang = RustLanguage;
        let content = r#"
[workspace]
members = ["crate1", "crate2"]
"#;
        assert!(lang.is_workspace_root("Cargo.toml", Some(content)));
    }

    #[test]
    fn test_is_workspace_root_false_package() {
        let lang = RustLanguage;
        let content = r#"
[package]
name = "myapp"
version = "0.1.0"
"#;
        assert!(!lang.is_workspace_root("Cargo.toml", Some(content)));
    }

    #[test]
    fn test_is_workspace_root_wrong_file() {
        let lang = RustLanguage;
        assert!(!lang.is_workspace_root("package.json", Some("[workspace]")));
    }

    #[test]
    fn test_is_workspace_root_no_content() {
        let lang = RustLanguage;
        assert!(!lang.is_workspace_root("Cargo.toml", None));
    }

    #[test]
    fn test_parse_dependencies_simple() {
        let lang = RustLanguage;
        let content = r#"
[package]
name = "myapp"

[dependencies]
tokio = "1.0"
serde = { version = "1.0", features = ["derive"] }
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 2);
        assert_eq!(deps.internal_deps.len(), 0);
        assert!(deps.external_deps.iter().any(|d| d.name == "tokio"));
        assert!(deps.external_deps.iter().any(|d| d.name == "serde"));
    }

    #[test]
    fn test_parse_dependencies_path() {
        let lang = RustLanguage;
        let content = r#"
[dependencies]
tokio = "1.0"
mylib = { path = "../mylib" }
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.internal_deps.len(), 1);
        assert_eq!(deps.internal_deps[0].name, "mylib");
        assert!(deps.internal_deps[0].is_internal);
    }

    #[test]
    fn test_parse_dependencies_workspace() {
        let lang = RustLanguage;
        let content = r#"
[workspace]
members = ["crate1", "crate2", "nested/crate3"]
"#;
        let deps = lang.parse_dependencies(content, &[]);

        assert_eq!(deps.internal_deps.len(), 3);
        assert!(deps.internal_deps.iter().any(|d| d.name == "crate1"));
        assert!(deps.internal_deps.iter().any(|d| d.name == "crate2"));
        assert!(deps.internal_deps.iter().any(|d| d.name == "crate3"));
    }
}
