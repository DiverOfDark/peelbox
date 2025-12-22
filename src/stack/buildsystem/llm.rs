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

    fn manifest_patterns(&self) -> &[ManifestPattern] {
        &[]
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
        BuildTemplate {
            build_image: "alpine:latest".to_string(),
            runtime_image: "alpine:latest".to_string(),
            build_packages: vec![],
            runtime_packages: vec![],
            build_commands: self
                .detected_info
                .lock()
                .unwrap()
                .as_ref()
                .map(|info| info.build_commands.clone())
                .unwrap_or_default(),
            cache_paths: self.cache_dirs(),
            artifacts: vec![],
            common_ports: vec![],
        }
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
