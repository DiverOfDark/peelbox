//! Rust language definition

use super::{BuildTemplate, DetectionResult, LanguageDefinition, ManifestPattern};

/// Rust language definition
pub struct RustLanguage;

impl LanguageDefinition for RustLanguage {
    fn name(&self) -> &str {
        "Rust"
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn manifest_files(&self) -> &[ManifestPattern] {
        &[ManifestPattern {
            filename: "Cargo.toml",
            build_system: "cargo",
            priority: 10,
        }]
    }

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> Option<DetectionResult> {
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
            build_system: "cargo".to_string(),
            confidence,
        })
    }

    fn build_template(&self, build_system: &str) -> Option<BuildTemplate> {
        if build_system != "cargo" {
            return None;
        }

        Some(BuildTemplate {
            build_image: "rust:1.75".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            build_packages: vec!["pkg-config".to_string(), "libssl-dev".to_string()],
            runtime_packages: vec!["ca-certificates".to_string(), "libssl3".to_string()],
            build_commands: vec!["cargo build --release".to_string()],
            cache_paths: vec![
                "target/".to_string(),
                "/usr/local/cargo/registry/".to_string(),
            ],
            artifacts: vec!["target/release/{project_name}".to_string()],
            common_ports: vec![8080],
        })
    }

    fn build_systems(&self) -> &[&str] {
        &["cargo"]
    }

    fn excluded_dirs(&self) -> &[&str] {
        &["target", ".cargo"]
    }

    fn workspace_configs(&self) -> &[&str] {
        &[]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        let lang = RustLanguage;
        assert_eq!(lang.name(), "Rust");
    }

    #[test]
    fn test_extensions() {
        let lang = RustLanguage;
        assert_eq!(lang.extensions(), &["rs"]);
    }

    #[test]
    fn test_manifest_files() {
        let lang = RustLanguage;
        let manifests = lang.manifest_files();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].filename, "Cargo.toml");
        assert_eq!(manifests[0].build_system, "cargo");
    }

    #[test]
    fn test_detect_cargo_toml() {
        let lang = RustLanguage;
        let result = lang.detect("Cargo.toml", None);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.build_system, "cargo");
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
    fn test_build_template() {
        let lang = RustLanguage;
        let template = lang.build_template("cargo");
        assert!(template.is_some());
        let t = template.unwrap();
        assert_eq!(t.build_image, "rust:1.75");
        assert_eq!(t.runtime_image, "debian:bookworm-slim");
        assert!(t.build_commands.contains(&"cargo build --release".to_string()));
    }

    #[test]
    fn test_build_template_invalid_system() {
        let lang = RustLanguage;
        let template = lang.build_template("maven");
        assert!(template.is_none());
    }

    #[test]
    fn test_build_systems() {
        let lang = RustLanguage;
        assert_eq!(lang.build_systems(), &["cargo"]);
    }
}
