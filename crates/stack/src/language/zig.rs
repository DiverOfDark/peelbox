//! Zig language definition

use super::{Dependency, DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition};

pub struct ZigLanguage;

impl LanguageDefinition for ZigLanguage {
    fn id(&self) -> crate::LanguageId {
        crate::LanguageId::Zig
    }

    fn extensions(&self) -> Vec<String> {
        vec!["zig".to_string()]
    }

    fn detect(
        &self,
        manifest_name: &str,
        _manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        match manifest_name {
            "build.zig" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Zig,
                confidence: 1.0,
            }),
            "build.zig.zon" => Some(DetectionResult {
                build_system: crate::BuildSystemId::Zig,
                confidence: 0.95,
            }),
            _ => None,
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["zig".to_string()]
    }

    fn excluded_dirs(&self) -> Vec<String> {
        vec!["zig-cache".to_string(), "zig-out".to_string()]
    }

    fn parse_dependencies(
        &self,
        manifest_content: &str,
        _all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        use regex::Regex;

        // Parse build.zig.zon format: .dependencies = .{ .name = .{ ... }, ... }
        let deps_block_re =
            Regex::new(r"\.dependencies\s*=\s*\.\{([\s\S]*?)\n\s*\}").expect("valid regex");
        let dep_name_re = Regex::new(r"\.(\w+)\s*=\s*\.\{").expect("valid regex");

        let mut external_deps = Vec::new();

        if let Some(caps) = deps_block_re.captures(manifest_content) {
            let block = &caps[1];
            for dep_cap in dep_name_re.captures_iter(block) {
                let name = dep_cap[1].to_string();
                external_deps.push(Dependency {
                    name,
                    version: None,
                    is_internal: false,
                });
            }
        }

        if external_deps.is_empty() {
            DependencyInfo::empty()
        } else {
            DependencyInfo {
                internal_deps: vec![],
                external_deps,
                detected_by: DetectionMethod::Deterministic,
            }
        }
    }

    fn find_entrypoints(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        repo_root: &std::path::Path,
        project_root: &std::path::Path,
        file_tree: &[std::path::PathBuf],
    ) -> Vec<String> {
        use regex::Regex;

        let main_pattern = Regex::new(r"pub\s+fn\s+main\s*\(").unwrap();
        let mut entrypoints = Vec::new();

        for file_path in file_tree {
            if file_path.starts_with(project_root)
                && file_path.extension().and_then(|s| s.to_str()) == Some("zig")
            {
                // Join repo_root before reading to handle relative paths correctly
                let full_path = if file_path.is_absolute() {
                    file_path.clone()
                } else {
                    repo_root.join(file_path)
                };

                if let Ok(content) = fs.read_to_string(&full_path) {
                    if main_pattern.is_match(&content) {
                        // Return relative path, not just basename
                        entrypoints.push(file_path.to_string_lossy().to_string());
                    }
                }
            }
        }

        entrypoints
    }

    fn is_runnable(
        &self,
        fs: &dyn peelbox_core::fs::FileSystem,
        repo_root: &std::path::Path,
        project_root: &std::path::Path,
        file_tree: &[std::path::PathBuf],
        _manifest_content: Option<&str>,
    ) -> bool {
        !self
            .find_entrypoints(fs, repo_root, project_root, file_tree)
            .is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peelbox_core::fs::{DirEntry, FileMetadata, FileType};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    struct MockFileSystem {
        files: HashMap<PathBuf, String>,
    }

    impl peelbox_core::fs::FileSystem for MockFileSystem {
        fn read_to_string(&self, path: &Path) -> Result<String, anyhow::Error> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not found"))
        }

        fn exists(&self, path: &Path) -> bool {
            self.files.contains_key(path)
        }

        fn is_file(&self, path: &Path) -> bool {
            self.exists(path)
        }

        fn is_dir(&self, _path: &Path) -> bool {
            false
        }

        fn read_dir(&self, _path: &Path) -> Result<Vec<DirEntry>, anyhow::Error> {
            Ok(vec![])
        }

        fn metadata(&self, path: &Path) -> Result<FileMetadata, anyhow::Error> {
            if self.exists(path) {
                Ok(FileMetadata {
                    size: 100,
                    file_type: FileType::File,
                })
            } else {
                Err(anyhow::anyhow!("not found"))
            }
        }

        fn read_bytes(&self, path: &Path, _max_bytes: usize) -> Result<Vec<u8>, anyhow::Error> {
            self.read_to_string(path).map(|s| s.into_bytes())
        }

        fn canonicalize(&self, path: &Path) -> Result<PathBuf, anyhow::Error> {
            Ok(path.to_path_buf())
        }
    }

    #[test]
    fn test_language_id() {
        let zig = ZigLanguage;
        assert_eq!(zig.id(), crate::LanguageId::Zig);
    }

    #[test]
    fn test_extensions() {
        let zig = ZigLanguage;
        assert_eq!(zig.extensions(), vec!["zig"]);
    }

    #[test]
    fn test_detect_build_zig() {
        let zig = ZigLanguage;
        let result = zig.detect("build.zig", None);

        assert!(result.is_some());
        let detection = result.unwrap();
        assert_eq!(detection.build_system, crate::BuildSystemId::Zig);
        assert_eq!(detection.confidence, 1.0);
    }

    #[test]
    fn test_detect_build_zig_zon() {
        let zig = ZigLanguage;
        let result = zig.detect("build.zig.zon", None);

        assert!(result.is_some());
        let detection = result.unwrap();
        assert_eq!(detection.build_system, crate::BuildSystemId::Zig);
        assert_eq!(detection.confidence, 0.95);
    }

    #[test]
    fn test_detect_unknown_file() {
        let zig = ZigLanguage;
        let result = zig.detect("Cargo.toml", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_compatible_build_systems() {
        let zig = ZigLanguage;
        assert_eq!(zig.compatible_build_systems(), vec!["zig"]);
    }

    #[test]
    fn test_excluded_dirs() {
        let zig = ZigLanguage;
        assert_eq!(zig.excluded_dirs(), vec!["zig-cache", "zig-out"]);
    }

    #[test]
    fn test_find_entrypoints_with_main() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("/repo/src/main.zig"),
            r#"const std = @import("std");
pub fn main() !void {
    std.debug.print("Hello\n", .{});
}"#
            .to_string(),
        );

        let zig = ZigLanguage;
        let repo_root = PathBuf::from("/repo");
        let project_root = PathBuf::from("");
        let file_tree = vec![PathBuf::from("src/main.zig")];

        let entrypoints = zig.find_entrypoints(&fs, &repo_root, &project_root, &file_tree);

        assert_eq!(entrypoints.len(), 1);
        assert_eq!(entrypoints[0], "src/main.zig");
    }

    #[test]
    fn test_find_entrypoints_no_main() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("/repo/src/lib.zig"),
            r#"pub fn add(a: i32, b: i32) i32 {
    return a + b;
}"#
            .to_string(),
        );

        let zig = ZigLanguage;
        let repo_root = PathBuf::from("/repo");
        let project_root = PathBuf::from("");
        let file_tree = vec![PathBuf::from("src/lib.zig")];

        let entrypoints = zig.find_entrypoints(&fs, &repo_root, &project_root, &file_tree);

        assert_eq!(entrypoints.len(), 0);
    }

    #[test]
    fn test_find_entrypoints_multiple_files() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("/repo/cmd/server/main.zig"),
            r#"pub fn main() !void {}"#.to_string(),
        );

        fs.files.insert(
            PathBuf::from("/repo/cmd/client/main.zig"),
            r#"pub fn main() !void {}"#.to_string(),
        );

        fs.files.insert(
            PathBuf::from("/repo/src/lib.zig"),
            r#"pub fn helper() void {}"#.to_string(),
        );

        let zig = ZigLanguage;
        let repo_root = PathBuf::from("/repo");
        let project_root = PathBuf::from("");
        let file_tree = vec![
            PathBuf::from("cmd/server/main.zig"),
            PathBuf::from("cmd/client/main.zig"),
            PathBuf::from("src/lib.zig"),
        ];

        let entrypoints = zig.find_entrypoints(&fs, &repo_root, &project_root, &file_tree);

        assert_eq!(entrypoints.len(), 2);
        assert!(entrypoints.contains(&"cmd/server/main.zig".to_string()));
        assert!(entrypoints.contains(&"cmd/client/main.zig".to_string()));
    }

    #[test]
    fn test_is_runnable_with_entrypoint() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("/repo/main.zig"),
            r#"pub fn main() !void {}"#.to_string(),
        );

        let zig = ZigLanguage;
        let repo_root = PathBuf::from("/repo");
        let project_root = PathBuf::from("");
        let file_tree = vec![PathBuf::from("main.zig")];

        assert!(zig.is_runnable(&fs, &repo_root, &project_root, &file_tree, None));
    }

    #[test]
    fn test_parse_dependencies_zon() {
        let zig = ZigLanguage;
        let content = r#".{
    .name = .app,
    .version = "1.0.0",
    .fingerprint = 0xc96e70cf94f3d177,
    .dependencies = .{
        .zap = .{
            .url = "https://github.com/zigzap/zap/archive/refs/tags/v0.2.0.tar.gz",
            .hash = "122043bd4f7f735tried80d3e3ae7e7c00c5db1bb7e8e5ba1ce6d51f2a73c6bef3",
        },
    },
    .paths = .{
        "build.zig",
        "build.zig.zon",
        "src",
    },
}"#;
        let deps = zig.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.external_deps[0].name, "zap");
        assert!(!deps.external_deps[0].is_internal);
    }

    #[test]
    fn test_parse_dependencies_zon_multiple() {
        let zig = ZigLanguage;
        let content = r#".{
    .name = .app,
    .dependencies = .{
        .zap = .{
            .url = "https://example.com/zap.tar.gz",
            .hash = "abc123",
        },
        .@"facil.io" = .{
            .url = "https://example.com/facilio.tar.gz",
            .hash = "def456",
        },
    },
}"#;
        let deps = zig.parse_dependencies(content, &[]);

        assert_eq!(deps.detected_by, DetectionMethod::Deterministic);
        // Only matches \w+ pattern, so "facil.io" with @"..." quoting won't match
        assert_eq!(deps.external_deps.len(), 1);
        assert_eq!(deps.external_deps[0].name, "zap");
    }

    #[test]
    fn test_parse_dependencies_empty() {
        let zig = ZigLanguage;
        let content = r#".{
    .name = .app,
    .dependencies = .{},
}"#;
        let deps = zig.parse_dependencies(content, &[]);
        assert_eq!(deps.external_deps.len(), 0);
    }

    #[test]
    fn test_parse_dependencies_no_deps_section() {
        let zig = ZigLanguage;
        let content = r#".{
    .name = .app,
}"#;
        let deps = zig.parse_dependencies(content, &[]);
        assert_eq!(deps.external_deps.len(), 0);
    }

    #[test]
    fn test_is_runnable_without_entrypoint() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("/repo/lib.zig"),
            r#"pub fn add(a: i32, b: i32) i32 { return a + b; }"#.to_string(),
        );

        let zig = ZigLanguage;
        let repo_root = PathBuf::from("/repo");
        let project_root = PathBuf::from("");
        let file_tree = vec![PathBuf::from("lib.zig")];

        assert!(!zig.is_runnable(&fs, &repo_root, &project_root, &file_tree, None));
    }
}
