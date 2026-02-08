use std::path::{Path, PathBuf};

use peelbox_core::fs::FileSystem;

use crate::{BuildSystemId, LanguageId};

use super::{profile::LanguageProfile, DetectionResult, LanguageDefinition};

static PROFILE: LanguageProfile = LanguageProfile {
    extensions: &["ts", "tsx", "js", "jsx"],
    excluded_dirs: &["vendor", ".deno"],
    runtime_id: crate::RuntimeId::Deno,
    default_port: None,
    env_var_patterns: &[],
    port_patterns: &[],
    health_check_patterns: &[],
    default_health_endpoints: &[],
};

pub struct DenoLanguage;

impl LanguageDefinition for DenoLanguage {
    fn id(&self) -> LanguageId {
        LanguageId::Deno
    }

    fn profile(&self) -> &LanguageProfile {
        &PROFILE
    }

    fn detect(
        &self,
        manifest_name: &str,
        _manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        if manifest_name == "deno.json" || manifest_name == "deno.jsonc" {
            Some(DetectionResult {
                build_system: BuildSystemId::Deno,
                confidence: 1.0,
            })
        } else {
            None
        }
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        vec!["Deno".to_string()]
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec!["deno.json".to_string(), "deno.jsonc".to_string()]
    }

    fn is_workspace_root(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        if manifest_name != "deno.json" && manifest_name != "deno.jsonc" {
            return false;
        }

        if let Some(content) = manifest_content {
            if content.contains("\"workspace\"") || content.contains("\"members\"") {
                return true;
            }
        }
        false
    }

    fn is_main_file(&self, _fs: &dyn FileSystem, file_path: &Path) -> bool {
        if let Some(name) = file_path.file_name().and_then(|n| n.to_str()) {
            return matches!(
                name,
                "main.ts" | "index.ts" | "mod.ts" | "server.ts" | "app.ts"
            );
        }
        false
    }

    fn default_entrypoint(&self, _build_system: &str) -> Option<String> {
        Some("main.ts".to_string())
    }

    fn is_runnable(
        &self,
        fs: &dyn FileSystem,
        _repo_root: &Path,
        project_root: &Path,
        _file_tree: &[PathBuf],
        _manifest_content: Option<&str>,
    ) -> bool {
        for name in &["main.ts", "index.ts", "mod.ts", "server.ts", "app.ts"] {
            if fs.exists(&project_root.join(name)) {
                return true;
            }
        }
        false
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
        let deno = DenoLanguage;
        assert_eq!(deno.id(), LanguageId::Deno);
    }

    #[test]
    fn test_extensions() {
        let deno = DenoLanguage;
        let extensions = deno.extensions();
        assert_eq!(extensions, vec!["ts", "tsx", "js", "jsx"]);
    }

    #[test]
    fn test_detect_deno_json() {
        let deno = DenoLanguage;
        let result = deno.detect("deno.json", None);

        assert!(result.is_some());
        let detection = result.unwrap();
        assert_eq!(detection.build_system, BuildSystemId::Deno);
        assert_eq!(detection.confidence, 1.0);
    }

    #[test]
    fn test_detect_deno_jsonc() {
        let deno = DenoLanguage;
        let result = deno.detect("deno.jsonc", None);

        assert!(result.is_some());
        let detection = result.unwrap();
        assert_eq!(detection.build_system, BuildSystemId::Deno);
        assert_eq!(detection.confidence, 1.0);
    }

    #[test]
    fn test_detect_unknown_file() {
        let deno = DenoLanguage;
        let result = deno.detect("package.json", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_compatible_build_systems() {
        let deno = DenoLanguage;
        assert_eq!(deno.compatible_build_systems(), vec!["Deno"]);
    }

    #[test]
    fn test_excluded_dirs() {
        let deno = DenoLanguage;
        let excluded = deno.excluded_dirs();
        assert!(excluded.contains(&"vendor".to_string()));
        assert!(excluded.contains(&".deno".to_string()));
    }

    #[test]
    fn test_workspace_configs() {
        let deno = DenoLanguage;
        let configs = deno.workspace_configs();
        assert_eq!(configs, vec!["deno.json", "deno.jsonc"]);
    }

    #[test]
    fn test_is_workspace_root_with_workspace() {
        let deno = DenoLanguage;
        let manifest_content = r#"{
            "workspace": ["packages/*"]
        }"#;

        assert!(deno.is_workspace_root("deno.json", Some(manifest_content)));
    }

    #[test]
    fn test_is_workspace_root_with_members() {
        let deno = DenoLanguage;
        let manifest_content = r#"{
            "members": ["app", "lib"]
        }"#;

        assert!(deno.is_workspace_root("deno.json", Some(manifest_content)));
    }

    #[test]
    fn test_is_workspace_root_without_workspace() {
        let deno = DenoLanguage;
        let manifest_content = r#"{
            "tasks": {
                "start": "deno run main.ts"
            }
        }"#;

        assert!(!deno.is_workspace_root("deno.json", Some(manifest_content)));
    }

    #[test]
    fn test_is_workspace_root_wrong_manifest() {
        let deno = DenoLanguage;
        assert!(!deno.is_workspace_root("package.json", None));
    }

    #[test]
    fn test_is_main_file() {
        let deno = DenoLanguage;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        assert!(deno.is_main_file(&fs, Path::new("main.ts")));
        assert!(deno.is_main_file(&fs, Path::new("index.ts")));
        assert!(deno.is_main_file(&fs, Path::new("mod.ts")));
        assert!(deno.is_main_file(&fs, Path::new("server.ts")));
        assert!(deno.is_main_file(&fs, Path::new("app.ts")));
        assert!(!deno.is_main_file(&fs, Path::new("utils.ts")));
    }

    #[test]
    fn test_default_entrypoint() {
        let deno = DenoLanguage;
        let entrypoint = deno.default_entrypoint("Deno");
        assert_eq!(entrypoint, Some("main.ts".to_string()));
    }

    #[test]
    fn test_is_runnable_with_main_ts() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("main.ts"),
            "export default function() {}".to_string(),
        );

        let deno = DenoLanguage;
        let repo_root = PathBuf::from("/repo");
        let project_root = PathBuf::from("");
        let file_tree = vec![PathBuf::from("main.ts")];

        assert!(deno.is_runnable(&fs, &repo_root, &project_root, &file_tree, None));
    }

    #[test]
    fn test_is_runnable_with_index_ts() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("index.ts"),
            "console.log('hello')".to_string(),
        );

        let deno = DenoLanguage;
        let repo_root = PathBuf::from("/repo");
        let project_root = PathBuf::from("");
        let file_tree = vec![PathBuf::from("index.ts")];

        assert!(deno.is_runnable(&fs, &repo_root, &project_root, &file_tree, None));
    }

    #[test]
    fn test_is_runnable_without_entrypoint() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("utils.ts"),
            "export function add(a: number, b: number) { return a + b; }".to_string(),
        );

        let deno = DenoLanguage;
        let repo_root = PathBuf::from("/repo");
        let project_root = PathBuf::from("");
        let file_tree = vec![PathBuf::from("utils.ts")];

        assert!(!deno.is_runnable(&fs, &repo_root, &project_root, &file_tree, None));
    }

    #[test]
    fn test_runtime_name() {
        let deno = DenoLanguage;
        assert_eq!(deno.runtime_name(), Some("Deno".to_string()));
    }
}
