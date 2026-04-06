use crate::ids::{BuildSystemId, LanguageId, RuntimeId};
use crate::traits::ManifestParser;
use crate::types::*;
use std::collections::BTreeMap;
use std::path::Path;

const ZIG: LanguageId = LanguageId::new("zig");
const ZIG_BS: BuildSystemId = BuildSystemId::new("zig");
const NATIVE: RuntimeId = RuntimeId::new("native");

pub struct ZigBuildParser;

impl ManifestParser for ZigBuildParser {
    fn filenames(&self) -> &[&str] {
        &["build.zig"]
    }

    fn parse(&self, path: &Path, content: &str) -> Option<Manifest> {
        if !content.contains("std.Build") && !content.contains("@import") {
            return None;
        }

        // Extract executable name from addExecutable call
        let exe_name = extract_executable_name(content).unwrap_or_else(|| "app".to_string());

        // Detect if build.zig uses old-style API (Zig < 0.13)
        // Old API: .root_source_file = .{ .path = "..." }
        // New API: .root_source_file = b.path("...") or .root_source_file = .{ .src_path = ... }
        let uses_old_api = content.contains(".path = ");

        // When using old API, install a compatible Zig version via tarball
        // since Wolfi only ships 0.13+
        let (packages, commands, env) = if uses_old_api {
            (
                vec!["build-base".into(), "ca-certificates".into(), "curl".into(), "xz".into()],
                vec![
                    "mkdir -p /opt/zig && curl -sSL https://ziglang.org/download/0.12.0/zig-linux-x86_64-0.12.0.tar.xz | tar -xJ -C /opt/zig --strip-components=1".into(),
                    "zig build -Doptimize=ReleaseSafe".into(),
                ],
                BTreeMap::from([(
                    "PATH".to_string(),
                    "/opt/zig:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                )]),
            )
        } else {
            (
                vec!["zig".into(), "build-base".into(), "ca-certificates".into()],
                vec!["zig build -Doptimize=ReleaseSafe".into()],
                BTreeMap::new(),
            )
        };

        Some(Manifest {
            path: path.to_path_buf(),
            language: ZIG,
            build_system: ZIG_BS,
            runtime: NATIVE,
            package: Some(Package {
                name: exe_name.clone(),
                version: None,
                is_application: true,
            }),
            workspace: None,
            dependencies: Vec::new(),
            build: BuildSpec {
                packages,
                commands,
                member_transform: None,
                env,
                cache_dirs: vec!["zig-cache".into()],
                artifacts: vec![("zig-out/bin/*".into(), "/app/".into())],
            },
            runtime_config: RuntimeSpec {
                packages: vec!["glibc".into(), "ca-certificates".into()],
                env: BTreeMap::new(),
                entrypoint: Some(format!("/app/{}", exe_name)),
                workdir: Some("/app".into()),
                ports: vec![],
                health_endpoint: None,
            },
        })
    }
}

/// Extract the executable name from a build.zig addExecutable call.
/// Looks for patterns like: `.name = "myapp"` in addExecutable calls.
fn extract_executable_name(content: &str) -> Option<String> {
    // Look for .name = "..." inside addExecutable blocks
    // Pattern: addExecutable(.{...name = "foo"...})
    let exe_start = content.find("addExecutable")?;
    let after = &content[exe_start..];

    // Find .name = "..." within the next ~500 chars
    let search_range = &after[..after.len().min(500)];
    let name_pos = search_range.find(".name = \"")?;
    let name_start = name_pos + ".name = \"".len();
    let remaining = &search_range[name_start..];
    let name_end = remaining.find('"')?;
    let name = &remaining[..name_end];

    if !name.is_empty() {
        Some(name.to_string())
    } else {
        None
    }
}

inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(ZigBuildParser))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_executable_name() {
        let content = r#"
const exe = b.addExecutable(.{
    .name = "myapp",
    .root_source_file = b.path("src/main.zig"),
});
"#;
        assert_eq!(extract_executable_name(content), Some("myapp".to_string()));
    }

    #[test]
    fn test_extract_executable_name_old_api() {
        let content = r#"
const exe = b.addExecutable(.{
    .name = "zig",
    .root_source_file = .{ .path = "src/main.zig" },
});
"#;
        assert_eq!(extract_executable_name(content), Some("zig".to_string()));
    }

    #[test]
    fn test_old_api_detection() {
        let old_content = r#".root_source_file = .{ .path = "src/main.zig" }"#;
        assert!(old_content.contains(".path = "));

        let new_content = r#".root_source_file = b.path("src/main.zig")"#;
        assert!(!new_content.contains(".path = "));
    }

    #[test]
    fn test_parse_old_api_build_zig() {
        let parser = ZigBuildParser;
        let content = r#"
const std = @import("std");
pub fn build(b: *std.Build) void {
    const exe = b.addExecutable(.{
        .name = "myapp",
        .root_source_file = .{ .path = "src/main.zig" },
    });
    b.installArtifact(exe);
}
"#;
        let manifest = parser.parse(Path::new("build.zig"), content).unwrap();
        assert!(manifest.build.commands[0].contains("curl"));
        assert!(manifest.build.commands[0].contains("0.12.0"));
        assert_eq!(
            manifest.build.commands[1],
            "zig build -Doptimize=ReleaseSafe"
        );
        assert_eq!(manifest.package.as_ref().unwrap().name, "myapp");
        assert_eq!(
            manifest.runtime_config.entrypoint,
            Some("/app/myapp".to_string())
        );
    }

    #[test]
    fn test_parse_new_api_build_zig() {
        let parser = ZigBuildParser;
        let content = r#"
const std = @import("std");
pub fn build(b: *std.Build) void {
    const exe = b.addExecutable(.{
        .name = "myapp",
        .root_source_file = b.path("src/main.zig"),
    });
    b.installArtifact(exe);
}
"#;
        let manifest = parser.parse(Path::new("build.zig"), content).unwrap();
        assert_eq!(manifest.build.commands.len(), 1);
        assert_eq!(
            manifest.build.commands[0],
            "zig build -Doptimize=ReleaseSafe"
        );
        assert!(manifest.build.packages.contains(&"zig".to_string()));
    }
}
