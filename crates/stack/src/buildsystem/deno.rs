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
        _repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let mut stacks = Vec::new();

        for path in file_tree {
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name == "deno.json" || file_name == "deno.jsonc" {
                    stacks.push(DetectionStack {
                        language: crate::LanguageId::Deno,
                        build_system: BuildSystemId::Deno,
                        confidence: 1.0,
                        manifest_path: path.clone(),
                        framework: None,
                        depth: 0,
                        is_workspace_root: self.is_workspace_root(None),
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
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        BuildTemplate {
            build_packages: vec!["deno".to_string()],
            build_commands: vec!["deno cache main.ts".to_string()],
            cache_paths: vec!["/deno-dir".to_string()],
            common_ports: vec![8000],
            build_env: vec![("DENO_DIR".to_string(), "/deno-dir".to_string())]
                .into_iter()
                .collect(),
            runtime_copy: vec![
                ("{project_name}".to_string(), ".".to_string()),
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
