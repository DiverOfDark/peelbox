use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::{BuildSystemId, DetectionStack, LanguageId};
use anyhow::Result;
use peelbox_core::fs::FileSystem;
use peelbox_llm::{ChatMessage, LLMClient, LLMRequest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

    pub fn populate_info(&self, manifest_path: &Path, fs: &dyn FileSystem) -> Result<()> {
        let manifest_name = manifest_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let mut content = fs
            .read_to_string(manifest_path)
            .unwrap_or_else(|_| String::new());

        // Truncate content to avoid context window explosion (max 20KB)
        if content.len() > 20_000 {
            content.truncate(20_000);
            content.push_str("\n... (truncated)");
        }

        let prompt = format!(
            r#"Analyze this build manifest and extract build system information. Respond with JSON ONLY.

Manifest: {}
Content:
{}

Return JSON ONLY with build configuration using Wolfi package names:
{{
  "name": "zig",
  "manifest_files": ["build.zig"],
  "build_commands": ["zig build -Doptimize=ReleaseSafe"],
  "cache_dirs": ["zig-cache", ".zig-cache"],
  "build_packages": ["zig"],
  "runtime_packages": ["glibc", "ca-certificates"],
  "artifacts": ["zig-out/bin/hello"],
  "common_ports": [],
  "confidence": 0.9
}}

Wolfi package name guidance:
- Always specify version-specific packages (e.g., nodejs-22, not nodejs)
- For Node.js, use packages like: nodejs-22, nodejs-20, nodejs-18
- For Python, use packages like: python-3.12, python-3.11, python-3.10
- For Java, use packages like: openjdk-21, openjdk-17, openjdk-11
- For common packages: glibc, ca-certificates, build-base, gcc, openssl-dev, pkgconf
- Leave packages empty if you're unsure - the build system will validate
"#,
            manifest_name, content
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.llm_client.chat(request))
        })?;

        let json_str = strip_markdown_fences(&response.content);
        let info: BuildSystemInfo = serde_json::from_str(json_str)?;

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

        let repo_name = repo_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-repo");

        let prompt = format!(
            r#"Analyze this repository to identify build system manifests. Respond with JSON ONLY.

Repository: {}
File tree summary:
{}

Identify manifest files and their build systems. Return JSON array ONLY.
CRITICAL: Exclude lockfiles (package-lock.json, Cargo.lock, yarn.lock, etc), logs, and output directories.

[
  {{
    "manifest_path": "build.zig",
    "build_system": "zig",
    "language": "Zig",
    "confidence": 0.85
  }}
]
"#,
            repo_name, summary
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.llm_client.chat(request))
        })?;

        let json_str = strip_markdown_fences(&response.content);
        let manifests: Vec<ManifestInfo> = serde_json::from_str(json_str)?;

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
        wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> BuildTemplate {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| {
                let mut build_packages = info.build_packages.clone();
                build_packages.retain(|p| wolfi_index.has_package(p));

                BuildTemplate {
                    build_packages,
                    build_commands: info.build_commands.clone(),
                    cache_paths: info.cache_dirs.clone(),
                    common_ports: info.common_ports.clone(),
                    build_env: std::collections::HashMap::new(),
                    runtime_copy: vec![],
                    runtime_env: std::collections::HashMap::new(),
                }
            })
            .unwrap_or_else(|| BuildTemplate {
                build_packages: vec![],
                build_commands: vec![],
                cache_paths: vec![],
                common_ports: vec![],
                build_env: std::collections::HashMap::new(),
                runtime_copy: vec![],
                runtime_env: std::collections::HashMap::new(),
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
        .filter(|p| {
            let filename = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            !is_lockfile(filename)
        })
        .take(50)
        .map(|p| p.display().to_string())
        .collect();

    let mut ext_counts = BTreeMap::new();
    for path in file_tree {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !is_lockfile(filename) {
                *ext_counts.entry(ext).or_insert(0) += 1;
            }
        }
    }

    format!(
        "Root files: {}\nExtensions: {:?}",
        root_files.join(", "),
        ext_counts
    )
}

fn is_lockfile(filename: &str) -> bool {
    matches!(
        filename,
        "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "Cargo.lock"
            | "go.sum"
            | "composer.lock"
            | "Gemfile.lock"
            | "poetry.lock"
            | "mix.lock"
            | "packages.lock.json"
            | "project.assets.json"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use peelbox_core::fs::RealFileSystem;
    use peelbox_llm::{MockLLMClient, MockResponse};

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
        assert_eq!(result[0].language, LanguageId::Custom("Zig".to_string()));
    }

    #[test]
    fn test_create_file_tree_summary_excludes_lockfiles() {
        let file_tree = vec![
            PathBuf::from("package.json"),
            PathBuf::from("yarn.lock"),
            PathBuf::from("Cargo.toml"),
            PathBuf::from("Cargo.lock"),
            PathBuf::from("src/main.rs"),
        ];
        let summary = create_file_tree_summary(&file_tree);
        assert!(summary.contains("package.json"));
        assert!(summary.contains("Cargo.toml"));
        assert!(!summary.contains("yarn.lock"));
        assert!(!summary.contains("Cargo.lock"));

        assert!(summary.contains("\"json\": 1"));
        assert!(summary.contains("\"toml\": 1"));
        assert!(summary.contains("\"rs\": 1"));
        assert!(!summary.contains("\"lock\""));
    }
}
