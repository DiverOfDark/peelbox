//! Bazel build system

use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct BazelBuildSystem;

/// Map a file extension to a LanguageId using static extension mapping.
/// This mapping mirrors what each LanguageDefinition::extensions() returns.
fn extension_to_language(ext: &str) -> Option<LanguageId> {
    match ext {
        "cc" | "cpp" | "cxx" | "c++" | "h" | "hpp" | "hxx" => Some(LanguageId::Cpp),
        "java" => Some(LanguageId::Java),
        "kt" | "kts" => Some(LanguageId::Kotlin),
        "py" | "pyi" | "pyw" => Some(LanguageId::Python),
        "go" => Some(LanguageId::Go),
        "rs" => Some(LanguageId::Rust),
        "js" | "jsx" | "mjs" | "cjs" => Some(LanguageId::JavaScript),
        "ts" | "tsx" => Some(LanguageId::TypeScript),
        _ => None,
    }
}

impl BuildSystem for BazelBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Bazel
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "WORKSPACE".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "WORKSPACE.bazel".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "BUILD".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "BUILD.bazel".to_string(),
                priority: 10,
            },
        ]
    }

    fn detect_all(
        &self,
        _repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        use std::collections::HashSet;

        // Pre-compute directory→language map in O(N) single pass.
        // Only records the first detected language per directory (first-wins).
        let mut dir_languages: HashMap<PathBuf, LanguageId> = HashMap::new();
        for path in file_tree {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(lang) = extension_to_language(ext) {
                    let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
                    dir_languages.entry(dir).or_insert(lang);
                }
            }
        }

        let mut detections = Vec::new();
        let mut processed_dirs = HashSet::new();

        for path in file_tree {
            let name = path.file_name().and_then(|n| n.to_str());

            // Check for all Bazel manifest files
            let is_bazel_manifest = matches!(
                name,
                Some("BUILD") | Some("BUILD.bazel") | Some("WORKSPACE") | Some("WORKSPACE.bazel")
            );

            if is_bazel_manifest {
                let dir = path.parent().unwrap_or(Path::new(""));

                // Deduplicate: skip if already processed this directory
                if processed_dirs.contains(dir) {
                    continue;
                }
                processed_dirs.insert(dir.to_path_buf());

                // O(1) language lookup from pre-computed map
                let lang_id = dir_languages
                    .get(dir)
                    .cloned()
                    .unwrap_or_else(|| LanguageId::Custom("Bazel".to_string()));

                detections.push(DetectionStack::new(
                    BuildSystemId::Bazel,
                    lang_id,
                    path.clone(),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _relative_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        let mut build_packages = vec!["build-base".to_string()];

        if wolfi_index.has_package("bazel-7") {
            build_packages.push("bazel-7".to_string());
        } else if wolfi_index.has_package("bazel") {
            build_packages.push("bazel".to_string());
        } else {
            build_packages.push("bazel-7".to_string());
        }

        // Bazel always requires a JDK to run, regardless of the project language.
        // Add a JDK and set JAVA_HOME so Bazel can find it.
        let mut build_env = std::collections::BTreeMap::new();
        if wolfi_index.has_package("openjdk-21-default-jdk") {
            build_packages.push("openjdk-21-default-jdk".to_string());
        } else if wolfi_index.has_package("openjdk-21") {
            build_packages.push("openjdk-21".to_string());
        }
        build_env.insert(
            "JAVA_HOME".to_string(),
            "/usr/lib/jvm/java-21-openjdk".to_string(),
        );

        BuildTemplate {
            build_packages,
            build_commands: vec!["bazel build //...".to_string()],
            cache_paths: vec!["~/.cache/bazel".to_string()],
            common_ports: vec![8080],
            build_env,
            runtime_copy: vec![(
                "bazel-bin/{project_name}".to_string(),
                "/app/{project_name}".to_string(),
            )],
            runtime_env: std::collections::BTreeMap::new(),
            runtime_workdir: Some("/app".to_string()),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        // Only cache the Bazel output user root directory.
        // bazel-bin and bazel-out are convenience symlinks created by Bazel
        // pointing into the output base; mounting them as cache directories
        // would prevent Bazel from creating these symlinks and cause build
        // failures inside containers.
        vec![".cache/bazel".to_string()]
    }

    fn parse_package_metadata(
        &self,
        manifest_content: &str,
    ) -> Result<(String, bool), anyhow::Error> {
        let re = regex::Regex::new(r#"name\s*=\s*"([^"]+)""#).unwrap();
        if let Some(cap) = re.captures(manifest_content) {
            if let Some(name) = cap.get(1) {
                return Ok((name.as_str().to_string(), true));
            }
        }
        Ok(("app".to_string(), true))
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
    fn test_manifest_patterns() {
        let bazel = BazelBuildSystem;
        let patterns = bazel.manifest_patterns();

        assert_eq!(patterns.len(), 4);
        assert!(patterns.iter().any(|p| p.filename == "WORKSPACE"));
        assert!(patterns.iter().any(|p| p.filename == "WORKSPACE.bazel"));
        assert!(patterns.iter().any(|p| p.filename == "BUILD"));
        assert!(patterns.iter().any(|p| p.filename == "BUILD.bazel"));
    }

    #[test]
    fn test_detect_build_file() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("BUILD")];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].build_system, BuildSystemId::Bazel);
    }

    #[test]
    fn test_detect_workspace_file() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("WORKSPACE")];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].build_system, BuildSystemId::Bazel);
    }

    #[test]
    fn test_detect_workspace_bazel_file() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("WORKSPACE.bazel")];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_deduplication_multiple_manifests() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![
            PathBuf::from("BUILD"),
            PathBuf::from("BUILD.bazel"),
            PathBuf::from("WORKSPACE"),
            PathBuf::from("WORKSPACE.bazel"),
        ];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        // All in same directory, should only create 1 detection
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_nested_projects() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![
            PathBuf::from("WORKSPACE"),
            PathBuf::from("subproject/BUILD"),
            PathBuf::from("another/BUILD.bazel"),
        ];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_language_detection_cpp() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("BUILD"), PathBuf::from("main.cc")];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result[0].language, LanguageId::Cpp);
    }

    #[test]
    fn test_language_detection_java() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("BUILD"), PathBuf::from("Main.java")];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result[0].language, LanguageId::Java);
    }

    #[test]
    fn test_language_detection_python() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("BUILD"), PathBuf::from("main.py")];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result[0].language, LanguageId::Python);
    }

    #[test]
    fn test_language_detection_go() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("BUILD"), PathBuf::from("main.go")];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result[0].language, LanguageId::Go);
    }

    #[test]
    fn test_language_detection_kotlin() {
        let bazel = BazelBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("BUILD"), PathBuf::from("Main.kt")];

        let result = bazel.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result[0].language, LanguageId::Kotlin);
    }

    #[test]
    fn test_cache_dirs() {
        let bazel = BazelBuildSystem;
        let cache_dirs = bazel.cache_dirs();

        assert_eq!(cache_dirs.len(), 1);
        assert!(cache_dirs.contains(&".cache/bazel".to_string()));
    }

    #[test]
    fn test_build_template_structure() {
        let bazel = BazelBuildSystem;
        let wolfi_index = WolfiPackageIndex::for_tests();

        let template = bazel.build_template(&wolfi_index, Path::new("."), Path::new("."), None);

        assert!(template.build_packages.iter().any(|p| p.contains("bazel")));
        assert_eq!(template.build_commands, vec!["bazel build //..."]);
        assert_eq!(template.runtime_workdir, Some("/app".to_string()));
    }

    #[test]
    fn test_parse_package_metadata() {
        let bazel = BazelBuildSystem;

        let content = r#"
            workspace(name = "my_project")

            java_binary(
                name = "my_app",
            )
        "#;

        let result = bazel.parse_package_metadata(content).unwrap();
        assert_eq!(result.0, "my_project");
        assert!(result.1);
    }

    #[test]
    fn test_parse_package_metadata_fallback() {
        let bazel = BazelBuildSystem;

        let content = "# Empty BUILD file";

        let result = bazel.parse_package_metadata(content).unwrap();
        assert_eq!(result.0, "app");
        assert!(result.1);
    }
}
