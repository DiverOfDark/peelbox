use anyhow::Result;
use peelbox_core::fs::FileSystem;
use std::path::{Path, PathBuf};

use crate::{buildsystem::BuildTemplate, BuildSystemId};

use super::{BuildSystem, DetectionStack, ManifestPattern};

pub struct DenoBuildSystem;

impl BuildSystem for DenoBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Deno
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        vec![
            ManifestPattern {
                filename: "deno.json".to_string(),
                priority: 10,
            },
            ManifestPattern {
                filename: "deno.jsonc".to_string(),
                priority: 10,
            },
        ]
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut stacks = Vec::new();

        for path in file_tree {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name == "deno.json" || file_name == "deno.jsonc" {
                    let manifest_content = fs.read_to_string(&repo_root.join(path)).ok();
                    let is_workspace_root = self.is_workspace_root(manifest_content.as_deref());

                    stacks.push(DetectionStack {
                        language: crate::LanguageId::Deno,
                        build_system: BuildSystemId::Deno,
                        confidence: 1.0,
                        manifest_path: path.clone(),
                        framework: None,
                        depth: 0,
                        is_workspace_root,
                    });
                }
            }
        }

        Ok(stacks)
    }

    fn build_template(
        &self,
        _wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _relative_path: &Path,
        manifest_content: Option<&str>,
    ) -> BuildTemplate {
        // Parse manifest to check if build task exists
        let has_build_task = if let Some(content) = manifest_content {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
                json["tasks"]["build"].as_str().is_some()
            } else {
                false
            }
        } else {
            false
        };

        let build_commands = if has_build_task {
            vec!["deno install".to_string(), "deno task build".to_string()]
        } else {
            vec!["deno install".to_string()]
        };

        BuildTemplate {
            build_packages: vec!["deno".to_string()],
            build_commands,
            cache_paths: vec!["/deno-dir".to_string()],
            common_ports: vec![8000],
            build_env: vec![("DENO_DIR".to_string(), "/deno-dir".to_string())]
                .into_iter()
                .collect(),
            runtime_copy: vec![
                ("{project_name}".to_string(), "/app".to_string()),
                ("/deno-dir".to_string(), "/deno-dir".to_string()),
            ],
            runtime_env: vec![("DENO_DIR".to_string(), "/deno-dir".to_string())]
                .into_iter()
                .collect(),
            runtime_workdir: Some("/app/".to_string()),
        }
    }

    fn cache_dirs(&self) -> Vec<String> {
        vec!["/deno-dir".to_string()]
    }

    fn is_workspace_root(&self, manifest_content: Option<&str>) -> bool {
        if let Some(content) = manifest_content {
            return content.contains("\"workspace\"") || content.contains("\"members\"");
        }
        false
    }

    fn workspace_configs(&self) -> Vec<String> {
        vec!["deno.json".to_string(), "deno.jsonc".to_string()]
    }

    fn parse_workspace_patterns(
        &self,
        manifest_content: &str,
    ) -> Result<Vec<String>, anyhow::Error> {
        let json: serde_json::Value = serde_json::from_str(manifest_content)?;

        if let Some(members) = json["workspace"]["members"].as_array() {
            return Ok(members
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect());
        }

        if let Some(members) = json["members"].as_array() {
            return Ok(members
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect());
        }

        if let Some(workspace) = json["workspace"].as_array() {
            return Ok(workspace
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect());
        }

        Ok(vec![])
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
    fn test_build_system_id() {
        let deno = DenoBuildSystem;
        assert_eq!(deno.id(), BuildSystemId::Deno);
    }

    #[test]
    fn test_manifest_patterns() {
        let deno = DenoBuildSystem;
        let patterns = deno.manifest_patterns();

        assert_eq!(patterns.len(), 2);
        assert!(patterns
            .iter()
            .any(|p| p.filename == "deno.json" && p.priority == 10));
        assert!(patterns
            .iter()
            .any(|p| p.filename == "deno.jsonc" && p.priority == 10));
    }

    #[test]
    fn test_detect_deno_json() {
        let deno = DenoBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("deno.json")];

        let result = deno.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, crate::LanguageId::Deno);
        assert_eq!(result[0].build_system, BuildSystemId::Deno);
        assert_eq!(result[0].confidence, 1.0);
        assert_eq!(result[0].manifest_path, PathBuf::from("deno.json"));
        assert!(!result[0].is_workspace_root);
    }

    #[test]
    fn test_detect_deno_jsonc() {
        let deno = DenoBuildSystem;
        let fs = MockFileSystem {
            files: HashMap::new(),
        };

        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("deno.jsonc")];

        let result = deno.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].language, crate::LanguageId::Deno);
        assert_eq!(result[0].build_system, BuildSystemId::Deno);
        assert_eq!(result[0].confidence, 1.0);
        assert_eq!(result[0].manifest_path, PathBuf::from("deno.jsonc"));
    }

    #[test]
    fn test_detect_workspace_root() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("/repo/deno.json"),
            r#"{
                "workspace": ["packages/*"]
            }"#
            .to_string(),
        );

        let deno = DenoBuildSystem;
        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("deno.json")];

        let result = deno.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].is_workspace_root);
    }

    #[test]
    fn test_detect_non_workspace_root() {
        let mut fs = MockFileSystem {
            files: HashMap::new(),
        };

        fs.files.insert(
            PathBuf::from("/repo/deno.json"),
            r#"{
                "tasks": {
                    "start": "deno run main.ts"
                }
            }"#
            .to_string(),
        );

        let deno = DenoBuildSystem;
        let repo_root = PathBuf::from("/repo");
        let file_tree = vec![PathBuf::from("deno.json")];

        let result = deno.detect_all(&repo_root, &file_tree, &fs).unwrap();
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_workspace_root);
    }

    #[test]
    fn test_build_template_default() {
        let deno = DenoBuildSystem;
        let wolfi_index = WolfiPackageIndex::for_tests();
        let service_path = Path::new("/app");
        let relative_path = Path::new(".");

        let template = deno.build_template(&wolfi_index, service_path, relative_path, None);

        assert_eq!(template.build_packages, vec!["deno"]);
        assert_eq!(template.build_commands, vec!["deno install"]);
        assert_eq!(template.cache_paths, vec!["/deno-dir"]);
        assert_eq!(template.common_ports, vec![8000]);
        assert_eq!(
            template.runtime_copy,
            vec![
                ("{project_name}".to_string(), "/app".to_string()),
                ("/deno-dir".to_string(), "/deno-dir".to_string())
            ]
        );
        assert_eq!(template.runtime_workdir, Some("/app/".to_string()));
    }

    #[test]
    fn test_build_template_with_build_task() {
        let deno = DenoBuildSystem;
        let wolfi_index = WolfiPackageIndex::for_tests();
        let service_path = Path::new("/app");
        let relative_path = Path::new(".");

        let manifest_content = r#"{
            "tasks": {
                "build": "deno run -A build.ts",
                "start": "deno run -A main.ts"
            }
        }"#;

        let template = deno.build_template(
            &wolfi_index,
            service_path,
            relative_path,
            Some(manifest_content),
        );

        assert_eq!(template.build_packages, vec!["deno"]);
        assert_eq!(
            template.build_commands,
            vec!["deno install", "deno task build"]
        );
        assert_eq!(template.cache_paths, vec!["/deno-dir"]);
    }

    #[test]
    fn test_build_template_without_build_task() {
        let deno = DenoBuildSystem;
        let wolfi_index = WolfiPackageIndex::for_tests();
        let service_path = Path::new("/app");
        let relative_path = Path::new(".");

        let manifest_content = r#"{
            "tasks": {
                "start": "deno run -A main.ts"
            }
        }"#;

        let template = deno.build_template(
            &wolfi_index,
            service_path,
            relative_path,
            Some(manifest_content),
        );

        assert_eq!(template.build_packages, vec!["deno"]);
        assert_eq!(template.build_commands, vec!["deno install"]);
    }

    #[test]
    fn test_cache_dirs() {
        let deno = DenoBuildSystem;
        assert_eq!(deno.cache_dirs(), vec!["/deno-dir"]);
    }

    #[test]
    fn test_is_workspace_root() {
        let deno = DenoBuildSystem;

        // With workspace field
        let content1 = r#"{ "workspace": ["packages/*"] }"#;
        assert!(deno.is_workspace_root(Some(content1)));

        // With members field
        let content2 = r#"{ "members": ["app", "lib"] }"#;
        assert!(deno.is_workspace_root(Some(content2)));

        // Without workspace
        let content3 = r#"{ "tasks": {} }"#;
        assert!(!deno.is_workspace_root(Some(content3)));

        // None
        assert!(!deno.is_workspace_root(None));
    }

    #[test]
    fn test_workspace_configs() {
        let deno = DenoBuildSystem;
        let configs = deno.workspace_configs();
        assert_eq!(configs, vec!["deno.json", "deno.jsonc"]);
    }

    #[test]
    fn test_parse_workspace_patterns_workspace_members() {
        let deno = DenoBuildSystem;
        let manifest = r#"{
            "workspace": {
                "members": ["packages/app", "packages/lib"]
            }
        }"#;

        let patterns = deno.parse_workspace_patterns(manifest).unwrap();
        assert_eq!(patterns, vec!["packages/app", "packages/lib"]);
    }

    #[test]
    fn test_parse_workspace_patterns_members() {
        let deno = DenoBuildSystem;
        let manifest = r#"{
            "members": ["app", "lib", "cli"]
        }"#;

        let patterns = deno.parse_workspace_patterns(manifest).unwrap();
        assert_eq!(patterns, vec!["app", "lib", "cli"]);
    }

    #[test]
    fn test_parse_workspace_patterns_workspace_array() {
        let deno = DenoBuildSystem;
        let manifest = r#"{
            "workspace": ["packages/*", "tools/*"]
        }"#;

        let patterns = deno.parse_workspace_patterns(manifest).unwrap();
        assert_eq!(patterns, vec!["packages/*", "tools/*"]);
    }

    #[test]
    fn test_parse_workspace_patterns_empty() {
        let deno = DenoBuildSystem;
        let manifest = r#"{
            "tasks": {
                "start": "deno run main.ts"
            }
        }"#;

        let patterns = deno.parse_workspace_patterns(manifest).unwrap();
        assert!(patterns.is_empty());
    }
}
