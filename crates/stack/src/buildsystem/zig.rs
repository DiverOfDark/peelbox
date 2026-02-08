use super::BuildSystem;
use crate::{BuildSystemId, BuildTemplate, DetectionStack, ManifestPattern};
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

pub struct ZigBuildSystem;

impl BuildSystem for ZigBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Zig
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "build.zig".to_string(),
                priority: 100,
            },
            ManifestPattern {
                filename: "build.zig.zon".to_string(),
                priority: 95,
            },
        ]
    }

    fn detect_all(
        &self,
        _repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> anyhow::Result<Vec<DetectionStack>> {
        use std::collections::HashMap;

        // Collect manifests per directory, preferring build.zig.zon (contains dependency info)
        let mut dir_manifests: HashMap<PathBuf, (PathBuf, f64)> = HashMap::new();

        for path in file_tree {
            let filename = path.file_name().and_then(|n| n.to_str());

            let (is_manifest, confidence, priority) = match filename {
                // build.zig.zon is preferred as manifest because it contains dependency metadata
                Some("build.zig.zon") => (true, 0.95, 1),
                Some("build.zig") => (true, 1.0, 0),
                _ => (false, 0.0, 0),
            };

            if is_manifest {
                let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();

                let entry = dir_manifests.entry(dir);
                entry
                    .and_modify(|(existing_path, existing_confidence)| {
                        let existing_priority =
                            if existing_path.file_name().and_then(|n| n.to_str())
                                == Some("build.zig.zon")
                            {
                                1
                            } else {
                                0
                            };
                        if priority > existing_priority {
                            *existing_path = path.clone();
                            *existing_confidence = confidence;
                        }
                    })
                    .or_insert((path.clone(), confidence));
            }
        }

        let results = dir_manifests
            .into_values()
            .map(|(manifest_path, confidence)| DetectionStack {
                language: crate::LanguageId::Zig,
                build_system: self.id(),
                framework: None,
                is_workspace_root: false,
                manifest_path,
                confidence,
                depth: 0,
            })
            .collect();

        Ok(results)
    }

    fn build_template(
        &self,
        _wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _relative_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        BuildTemplate {
            build_packages: vec!["zig".to_string(), "build-base".to_string()],
            build_commands: vec!["zig build -Doptimize=ReleaseSafe".to_string()],
            build_env: Default::default(),
            cache_paths: vec!["zig-cache".to_string(), "zig-out".to_string()],
            common_ports: vec![],
            runtime_copy: vec![
                (
                    "zig-out/bin/{project_name}".to_string(),
                    "/app/{project_name}".to_string(),
                ),
                ("zig-out/lib/".to_string(), "/app/lib/".to_string()),
            ],
            runtime_env: Default::default(),
            runtime_workdir: Some("/app".to_string()),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["zig-cache".to_string(), "zig-out".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peelbox_core::fs::{DirEntry, FileMetadata, FileType};
    use peelbox_wolfi::WolfiPackageIndex;
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
    fn test_detect_build_zig() {
        let zig = ZigBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("build.zig")];

        let result = zig.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, crate::LanguageId::Zig);
        assert_eq!(result[0].build_system, BuildSystemId::Zig);
        assert_eq!(result[0].confidence, 1.0);
        assert_eq!(result[0].manifest_path, PathBuf::from("build.zig"));
    }

    #[test]
    fn test_detect_build_zig_zon() {
        let zig = ZigBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("build.zig.zon")];

        let result = zig.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, crate::LanguageId::Zig);
        assert_eq!(result[0].build_system, BuildSystemId::Zig);
        assert_eq!(result[0].confidence, 0.95);
        assert_eq!(result[0].manifest_path, PathBuf::from("build.zig.zon"));
    }

    #[test]
    fn test_detect_both_manifests_same_dir() {
        let zig = ZigBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("build.zig"), PathBuf::from("build.zig.zon")];

        let result = zig.detect_all(&repo_root, &file_tree, &fs).unwrap();
        // Should only detect once per directory (deduplication), preferring build.zig.zon
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, crate::LanguageId::Zig);
        assert_eq!(result[0].manifest_path, PathBuf::from("build.zig.zon"));
    }

    #[test]
    fn test_detect_nested_projects() {
        let zig = ZigBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![
            PathBuf::from("build.zig"),
            PathBuf::from("subproject/build.zig"),
        ];

        let result = zig.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 2);
        let paths: Vec<_> = result.iter().map(|r| r.manifest_path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("build.zig")));
        assert!(paths.contains(&PathBuf::from("subproject/build.zig")));
    }

    #[test]
    fn test_build_template_generation() {
        let zig = ZigBuildSystem;
        let wolfi_index = WolfiPackageIndex::for_tests();

        let template = zig.build_template(&wolfi_index, Path::new("."), Path::new("."), None);

        assert_eq!(template.build_packages, vec!["zig", "build-base"]);
        assert_eq!(
            template.build_commands,
            vec!["zig build -Doptimize=ReleaseSafe"]
        );
        assert_eq!(template.cache_paths, vec!["zig-cache", "zig-out"]);
        assert_eq!(
            template.runtime_copy,
            vec![
                (
                    "zig-out/bin/{project_name}".to_string(),
                    "/app/{project_name}".to_string()
                ),
                ("zig-out/lib/".to_string(), "/app/lib/".to_string()),
            ]
        );
        assert_eq!(template.runtime_workdir, Some("/app".to_string()));
    }

    #[test]
    fn test_cache_dirs() {
        let zig = ZigBuildSystem;
        let cache_dirs = zig.cache_dirs();

        assert_eq!(cache_dirs, vec!["zig-cache", "zig-out"]);
        // Ensure no duplicates
        assert_eq!(cache_dirs.len(), 2);
    }

    #[test]
    fn test_manifest_patterns() {
        let zig = ZigBuildSystem;
        let patterns = zig.manifest_patterns();

        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0].filename, "build.zig");
        assert_eq!(patterns[0].priority, 100);
        assert_eq!(patterns[1].filename, "build.zig.zon");
        assert_eq!(patterns[1].priority, 95);
    }
}
