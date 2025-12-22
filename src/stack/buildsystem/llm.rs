use super::{BuildSystem, BuildTemplate, ManifestPattern};
use crate::llm::{ChatMessage, LLMClient, LLMRequest};
use crate::stack::BuildSystemId;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BuildSystemInfo {
    name: String,
    manifest_files: Vec<String>,
    build_commands: Vec<String>,
    cache_dirs: Vec<String>,
    build_image: String,
    runtime_image: String,
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

    fn detect(&self, manifest_name: &str, manifest_content: Option<&str>) -> bool {
        let content_preview = manifest_content
            .map(|c| c.chars().take(500).collect::<String>())
            .unwrap_or_default();

        let prompt = format!(
            r#"Analyze this build manifest to identify the build system. Respond with JSON only.

File: {}
Content:
{}

Response format:
{{
  "name": "BuildSystemName",
  "manifest_files": ["file1.ext"],
  "build_commands": ["build", "test"],
  "cache_dirs": [".cache"],
  "build_image": "base-image:tag",
  "runtime_image": "runtime-image:tag",
  "build_packages": ["gcc", "make"],
  "runtime_packages": ["libc"],
  "artifacts": ["/app/build/*"],
  "common_ports": [8080, 3000],
  "confidence": 0.95
}}
"#,
            manifest_name, content_preview
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.llm_client.chat(request))
        }) {
            Ok(resp) => resp,
            Err(_) => return false,
        };

        let info: BuildSystemInfo = match serde_json::from_str(&response.content) {
            Ok(i) => i,
            Err(_) => return false,
        };

        if info.confidence < 0.5 {
            return false;
        }

        *self.detected_info.lock().unwrap() = Some(info);
        true
    }

    fn build_template(&self) -> BuildTemplate {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| BuildTemplate {
                build_image: info.build_image.clone(),
                runtime_image: info.runtime_image.clone(),
                build_packages: info.build_packages.clone(),
                runtime_packages: info.runtime_packages.clone(),
                build_commands: info.build_commands.clone(),
                cache_paths: info.cache_dirs.clone(),
                artifacts: info.artifacts.clone(),
                common_ports: info.common_ports.clone(),
            })
            .unwrap_or_else(|| BuildTemplate {
                build_image: "alpine:latest".to_string(),
                runtime_image: "alpine:latest".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn test_llm_build_system_detect_success() {
        let info = BuildSystemInfo {
            name: "Bazel".to_string(),
            manifest_files: vec!["BUILD".to_string()],
            build_commands: vec!["bazel build".to_string()],
            cache_dirs: vec!["bazel-out".to_string()],
            build_image: "ubuntu:22.04".to_string(),
            runtime_image: "ubuntu:22.04".to_string(),
            build_packages: vec!["bazel".to_string()],
            runtime_packages: vec![],
            artifacts: vec!["/app/bazel-bin/*".to_string()],
            common_ports: vec![8080],
            confidence: 0.9,
        };

        let json = serde_json::to_string(&info).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let build_system = LLMBuildSystem::new(client);
        let result = build_system.detect("BUILD", Some("load(\"@bazel_tools\")"));

        assert!(result);
        assert_eq!(
            build_system.id(),
            BuildSystemId::Custom("Bazel".to_string())
        );
        assert_eq!(build_system.cache_dirs(), vec!["bazel-out"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_confidence_validation() {
        let low_confidence = BuildSystemInfo {
            name: "Unknown".to_string(),
            manifest_files: vec![],
            build_commands: vec![],
            cache_dirs: vec![],
            build_image: "alpine:latest".to_string(),
            runtime_image: "alpine:latest".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            artifacts: vec![],
            common_ports: vec![],
            confidence: 0.2,
        };

        let json = serde_json::to_string(&low_confidence).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let build_system = LLMBuildSystem::new(client);
        let result = build_system.detect("unknown.txt", Some("content"));

        assert!(!result);
    }
}
