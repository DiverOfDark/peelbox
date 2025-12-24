use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::fs::FileSystem;
use crate::llm::{ChatMessage, LLMClient, LLMRequest};
use crate::stack::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestInfo {
    manifest_path: String,
    build_system: String,
    language: String,
    confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuildSystemInfo {
    name: String,
    manifest_files: Vec<String>,
    build_commands: Vec<String>,
    cache_dirs: Vec<String>,
    build_packages: Vec<String>,
    runtime_packages: Vec<String>,
    artifacts: Vec<String>,
    common_ports: Vec<u16>,
    confidence: f32,
}

pub struct LLMBuildSystem {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<BuildSystemInfo>>>,
}

impl LLMBuildSystem {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            detected_info: Arc::new(Mutex::new(None)),
        }
    }

    pub fn populate_info(
        &self,
        manifest_path: &Path,
        fs: &dyn FileSystem,
    ) -> Result<()> {
        let manifest_name = manifest_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let content = fs.read_to_string(manifest_path)
            .unwrap_or_else(|_| String::new());

        let prompt = format!(
            r#"Analyze this build manifest and extract build system information.

Manifest: {}
Content:
{}

Return JSON with build configuration:
{{
  "name": "zig",
  "manifest_files": ["build.zig"],
  "build_commands": ["zig build -Doptimize=ReleaseSafe"],
  "cache_dirs": ["zig-cache", ".zig-cache"],
  "build_image": "alpine:latest",
  "runtime_image": "alpine:latest",
  "build_packages": [],
  "runtime_packages": [],
  "artifacts": ["zig-out/bin/hello"],
  "common_ports": [],
  "confidence": 0.9
}}
"#,
            manifest_name,
            content
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.llm_client.chat(request))
        })?;

        let json_str = strip_markdown_fences(&response.content);
        let info: BuildSystemInfo = serde_json::from_str(&json_str)?;

        *self.detected_info.lock().unwrap() = Some(info);

        Ok(())
    }
}

impl BuildSystem for LLMBuildSystem {
    fn id(&self) -> BuildSystemId {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| BuildSystemId::Custom(info.name.clone()))
            .unwrap_or_else(|| BuildSystemId::Custom("Unknown".to_string()))
    }

    fn manifest_patterns(&self) -> Vec<ManifestPattern> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| {
                info.manifest_files
                    .iter()
                    .map(|name| ManifestPattern {
                        filename: name.clone(),
                        priority: 50,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn detect_all(
        &self,
        repo_root: &Path,
        file_tree: &[PathBuf],
        _fs: &dyn FileSystem,
    ) -> Result<Vec<DetectionStack>> {
        let summary = create_file_tree_summary(file_tree);

        let prompt = format!(
            r#"Analyze this repository to identify build system manifests.

Repository: {}
File tree summary:
{}

Identify manifest files and their build systems. Return JSON array:
[{{
  "manifest_path": "build.zig",
  "build_system": "zig",
  "language": "Zig",
  "confidence": 0.85
}}]
"#,
            repo_root.display(),
            summary
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.llm_client.chat(request))
        })?;

        let json_str = strip_markdown_fences(&response.content);
        let manifests: Vec<ManifestInfo> = serde_json::from_str(&json_str)?;

        let mut detections = Vec::new();
        for manifest in manifests {
            if manifest.confidence >= 0.5 {
                detections.push(DetectionStack::new(
                    BuildSystemId::Custom(manifest.build_system),
                    LanguageId::Custom(manifest.language),
                    PathBuf::from(manifest.manifest_path),
                ));
            }
        }

        Ok(detections)
    }

    fn build_template(
        &self,
        wolfi_index: &crate::validation::WolfiPackageIndex,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| {
                let mut build_packages = info.build_packages.clone();
                let mut runtime_packages = info.runtime_packages.clone();

                build_packages.retain(|p| wolfi_index.has_package(p));
                runtime_packages.retain(|p| wolfi_index.has_package(p));

                BuildTemplate {
                    build_packages,
                    runtime_packages,
                    build_commands: info.build_commands.clone(),
                    cache_paths: info.cache_dirs.clone(),
                    artifacts: info.artifacts.clone(),
                    common_ports: info.common_ports.clone(),
                }
            })
            .unwrap_or_else(|| BuildTemplate {
                build_packages: vec![],
                runtime_packages: vec![],
                build_commands: vec![],
                cache_paths: vec![],
                artifacts: vec![],
                common_ports: vec![],
            })
    }

    fn cache_dirs(&self) -> Vec<String> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| info.cache_dirs.clone())
            .unwrap_or_default()
    }
}

fn strip_markdown_fences(content: &str) -> &str {
    let trimmed = content.trim();
    if trimmed.starts_with("```json") {
        trimmed
            .strip_prefix("```json")
            .unwrap_or(trimmed)
            .strip_suffix("```")
            .unwrap_or(trimmed)
            .trim()
    } else if trimmed.starts_with("```") {
        trimmed
            .strip_prefix("```")
            .unwrap_or(trimmed)
            .strip_suffix("```")
            .unwrap_or(trimmed)
            .trim()
    } else {
        trimmed
    }
}

fn create_file_tree_summary(file_tree: &[PathBuf]) -> String {
    let root_files: Vec<_> = file_tree
        .iter()
        .filter(|p| p.components().count() == 1)
        .take(50)
        .map(|p| p.display().to_string())
        .collect();

    let mut ext_counts = HashMap::new();
    for path in file_tree {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            *ext_counts.entry(ext).or_insert(0) += 1;
        }
    }

    format!(
        "Root files: {}\nExtensions: {:?}",
        root_files.join(", "),
        ext_counts
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::RealFileSystem;
    use crate::llm::{MockLLMClient, MockResponse};

    #[test]
    fn test_llm_build_system_id_default() {
        let client = Arc::new(MockLLMClient::new());
        let build_system = LLMBuildSystem::new(client);
        assert_eq!(
            build_system.id(),
            BuildSystemId::Custom("Unknown".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_llm_build_system_detect_all() {
        let manifests = vec![ManifestInfo {
            manifest_path: "build.zig".to_string(),
            build_system: "zig".to_string(),
            language: "Zig".to_string(),
            confidence: 0.9,
        }];

        let json = serde_json::to_string(&manifests).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let build_system = LLMBuildSystem::new(client);
        let file_tree = vec![PathBuf::from("build.zig"), PathBuf::from("src/main.zig")];
        let fs = RealFileSystem;

        let result = build_system
            .detect_all(Path::new("/tmp"), &file_tree, &fs)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].build_system,
            BuildSystemId::Custom("zig".to_string())
        );
        assert_eq!(
            result[0].language,
            LanguageId::Custom("Zig".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_confidence_filtering() {
        let manifests = vec![
            ManifestInfo {
                manifest_path: "build.zig".to_string(),
                build_system: "zig".to_string(),
                language: "Zig".to_string(),
                confidence: 0.9,
            },
            ManifestInfo {
                manifest_path: "unknown.txt".to_string(),
                build_system: "Unknown".to_string(),
                language: "Unknown".to_string(),
                confidence: 0.2,
            },
        ];

        let json = serde_json::to_string(&manifests).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let build_system = LLMBuildSystem::new(client);
        let file_tree = vec![
            PathBuf::from("build.zig"),
            PathBuf::from("unknown.txt"),
        ];
        let fs = RealFileSystem;

        let result = build_system
            .detect_all(Path::new("/tmp"), &file_tree, &fs)
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].build_system,
            BuildSystemId::Custom("zig".to_string())
        );
    }
}
