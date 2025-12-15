//! Rust language definition

use super::{
    BuildTemplate, Dependency, DependencyInfo, DetectionMethod, DetectionResult,
    LanguageDefinition, ManifestPattern,
};
use std::collections::HashSet;

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
        _all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        let parsed: toml::Value = match toml::from_str(manifest_content) {
            Ok(v) => v,
            Err(_) => return DependencyInfo::empty(),
        };

        let mut internal_deps = Vec::new();
        let mut external_deps = Vec::new();
        let mut seen = HashSet::new();

        for dep_section in &["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(deps) = parsed.get(dep_section).and_then(|v| v.as_table()) {
                for (name, value) in deps {
                    if seen.contains(name) {
                        continue;
                    }
                    seen.insert(name.clone());

                    let (version, is_internal) = if let Some(table) = value.as_table() {
                        let version = table
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let is_path = table.get("path").is_some();
                        (version, is_path)
                    } else if let Some(ver) = value.as_str() {
                        (Some(ver.to_string()), false)
                    } else {
                        (None, false)
                    };

                    let dep = Dependency {
                        name: name.clone(),
                        version,
                        is_internal,
                    };

                    if is_internal {
                        internal_deps.push(dep);
                    } else {
                        external_deps.push(dep);
                    }
                }
            }
        }

        if let Some(workspace) = parsed.get("workspace").and_then(|v| v.as_table()) {
            if let Some(members) = workspace.get("members").and_then(|v| v.as_array()) {
                for member in members {
                    if let Some(member_name) = member.as_str() {
                        let name = member_name
                            .split('/')
                            .next_back()
                            .unwrap_or(member_name)
                            .to_string();
                        if !seen.contains(&name) {
                            internal_deps.push(Dependency {
                                name: name.clone(),
                                version: Some("workspace".to_string()),
                                is_internal: true,
                            });
                            seen.insert(name);
                        }
                    }
                }
            }
        }

        DependencyInfo {
            internal_deps,
            external_deps,
            detected_by: DetectionMethod::Deterministic,
        }
    }

    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r#"std::env::var\(["']([A-Z_][A-Z0-9_]*)["']"#, "std::env"),
            (r#"env::var\(["']([A-Z_][A-Z0-9_]*)["']"#, "env::var"),
        ]
    }

    fn port_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r"\.bind\([^,)]*:(\d{4,5})", "bind()"),
            (r#"addr\s*=\s*"[^:]*:(\d{4,5})""#, "addr config"),
        ]
    }

    fn health_check_patterns(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (r#"\.route\(['"]([/\w\-]*health[/\w\-]*)['"]"#, "axum/actix"),
            (r#"\.get\(['"]([/\w\-]*health[/\w\-]*)['"]"#, "rocket/warp"),
        ]
    }

    fn is_main_file(&self, fs: &dyn crate::fs::FileSystem, file_path: &std::path::Path) -> bool {
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

    fn default_health_endpoints(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }

    fn default_env_vars(&self) -> Vec<&'static str> {
        vec![]
    }

    fn runtime_name(&self) -> Option<&'static str> {
        Some("rust")
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
        assert!(t
            .build_commands
            .contains(&"cargo build --release".to_string()));
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
