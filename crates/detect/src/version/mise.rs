//! Mise (formerly rtx) and `.tool-versions` (asdf) configuration scanning.
//!
//! Reads tool version declarations from:
//! - `mise.toml` / `.mise.toml` (TOML format with `[tools]` section)
//! - `.tool-versions` (asdf-compatible `tool_name version` format)
//!
//! After reading, maps tool names to Wolfi package overrides in the build.

use crate::helpers::{
    extract_major_minor_version, extract_major_version, extract_project_dir, replace_package,
};
use peelbox_core::output::schema::UniversalBuild;
use std::path::Path;
use tracing::debug;

/// Scan for mise.toml / .mise.toml / .tool-versions and add any additional
/// tools that the project declares as build/runtime packages.
///
/// This handles the common pattern where a project uses mise (formerly rtx)
/// to manage multiple language runtimes (Node, Python, Go, Bun, etc.).
pub fn scan_mise_config(repo_root: &Path, build: &mut UniversalBuild) {
    let project_dir = extract_project_dir(repo_root, &build.metadata.reasoning);

    // Try mise.toml / .mise.toml first, then .tool-versions
    let tools = read_mise_tools(&project_dir, repo_root);
    if tools.is_empty() {
        return;
    }

    debug!(?tools, "Found mise/tool-versions config");

    // Map mise tool names to Wolfi package names
    for (tool, version) in &tools {
        match tool.as_str() {
            "node" | "nodejs" => {
                // Node version override (mise takes priority over .nvmrc)
                let major = extract_major_version(version);
                if let Some(major) = major {
                    let versioned_pkg = format!("nodejs-{}", major);
                    replace_package(&mut build.build.packages, "nodejs", &versioned_pkg);
                    replace_package(&mut build.runtime.packages, "nodejs", &versioned_pkg);
                }
            }
            "bun" => {
                // Add bun as both build and runtime package
                if !build.build.packages.iter().any(|p| p.starts_with("bun")) {
                    build.build.packages.push("bun".to_string());
                }
                if !build.runtime.packages.iter().any(|p| p.starts_with("bun")) {
                    build.runtime.packages.push("bun".to_string());
                }
            }
            "python" => {
                // Add python as runtime package if not already present
                let major_minor = extract_major_minor_version(version);
                let pkg_name = if let Some(mm) = major_minor {
                    format!("python-{}", mm)
                } else {
                    "python".to_string()
                };
                if !build
                    .runtime
                    .packages
                    .iter()
                    .any(|p| p.starts_with("python"))
                {
                    build.runtime.packages.push(pkg_name);
                }
            }
            "go" | "golang" => {
                // Add go as runtime package if not already present
                if !build.runtime.packages.iter().any(|p| p.starts_with("go-"))
                    && !build.runtime.packages.contains(&"go".to_string())
                {
                    build.runtime.packages.push("go".to_string());
                }
            }
            "ruby" => {
                let major_minor = extract_major_minor_version(version);
                if let Some(mm) = major_minor {
                    let versioned_pkg = format!("ruby-{}", mm);
                    replace_package(&mut build.build.packages, "ruby", &versioned_pkg);
                    replace_package(&mut build.runtime.packages, "ruby", &versioned_pkg);
                }
            }
            _ => {
                // Unknown tool -- skip
            }
        }
    }
}

/// Read tool versions from mise.toml / .mise.toml / .tool-versions.
/// Returns a list of (tool_name, version_string) pairs.
pub fn read_mise_tools(project_dir: &Path, repo_root: &Path) -> Vec<(String, String)> {
    for dir in &[project_dir, repo_root] {
        // Try TOML-based mise config first
        for filename in &["mise.toml", ".mise.toml"] {
            let path = dir.join(filename);
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(tools) = parse_mise_toml(&content) {
                    return tools;
                }
            }
        }

        // Try .tool-versions (asdf-compatible format)
        let path = dir.join(".tool-versions");
        if let Ok(content) = std::fs::read_to_string(&path) {
            return parse_tool_versions(&content);
        }
    }

    Vec::new()
}

/// Parse mise.toml / .mise.toml content.
/// Format:
/// ```toml
/// [tools]
/// node = "22"
/// python = "3.12"
/// bun = "latest"
/// ```
pub fn parse_mise_toml(content: &str) -> Option<Vec<(String, String)>> {
    let toml_val: toml::Value = toml::from_str(content).ok()?;
    let tools = toml_val.get("tools").and_then(|v| v.as_table())?;

    let mut result = Vec::new();
    for (name, value) in tools {
        let version = match value {
            toml::Value::String(s) => s.clone(),
            toml::Value::Integer(i) => i.to_string(),
            toml::Value::Float(f) => f.to_string(),
            _ => continue,
        };
        result.push((name.clone(), version));
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Parse .tool-versions (asdf-compatible) content.
/// Format: `tool_name version [version2 ...]`
pub fn parse_tool_versions(content: &str) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        if let (Some(tool), Some(version)) = (parts.next(), parts.next()) {
            result.push((tool.to_string(), version.to_string()));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mise_toml() {
        let content = r#"
[tools]
node = "22"
python = "3.12"
"#;
        let tools = parse_mise_toml(content).unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&("node".to_string(), "22".to_string())));
        assert!(tools.contains(&("python".to_string(), "3.12".to_string())));
    }

    #[test]
    fn test_parse_mise_toml_empty() {
        let content = r#"
[tools]
"#;
        assert_eq!(parse_mise_toml(content), None);
    }

    #[test]
    fn test_parse_mise_toml_no_tools_section() {
        let content = r#"
[settings]
experimental = true
"#;
        assert_eq!(parse_mise_toml(content), None);
    }

    #[test]
    fn test_parse_tool_versions() {
        let content = "\
node 22.1.0
python 3.12.1
ruby 3.2.2
";
        let tools = parse_tool_versions(content);
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0], ("node".to_string(), "22.1.0".to_string()));
        assert_eq!(tools[1], ("python".to_string(), "3.12.1".to_string()));
        assert_eq!(tools[2], ("ruby".to_string(), "3.2.2".to_string()));
    }

    #[test]
    fn test_parse_tool_versions_with_comments() {
        let content = "\
# Node.js version
node 22.1.0

# Python version
python 3.12.1
";
        let tools = parse_tool_versions(content);
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_read_mise_tools_from_mise_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mise.toml"), "[tools]\nnode = \"22\"\n").unwrap();
        let tools = read_mise_tools(dir.path(), dir.path());
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0], ("node".to_string(), "22".to_string()));
    }

    #[test]
    fn test_read_mise_tools_from_tool_versions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".tool-versions"), "node 22.1.0\n").unwrap();
        let tools = read_mise_tools(dir.path(), dir.path());
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0], ("node".to_string(), "22.1.0".to_string()));
    }
}
