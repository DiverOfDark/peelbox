use super::{DependencyInfo, DetectionMethod, DetectionResult, LanguageDefinition};
use crate::llm::{ChatMessage, LLMClient, LLMRequest};
use crate::stack::LanguageId;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LanguageInfo {
    name: String,
    file_extensions: Vec<String>,
    package_managers: Vec<String>,
    confidence: f32,
}

pub struct LLMLanguage {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<LanguageInfo>>>,
}

impl LLMLanguage {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            detected_info: Arc::new(Mutex::new(None)),
        }
    }
}

impl LanguageDefinition for LLMLanguage {
    fn id(&self) -> LanguageId {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| LanguageId::Custom(info.name.clone()))
            .unwrap_or_else(|| LanguageId::Custom("Unknown".to_string()))
    }

    fn extensions(&self) -> &[&str] {
        &[]
    }

    fn detect(
        &self,
        manifest_name: &str,
        manifest_content: Option<&str>,
    ) -> Option<DetectionResult> {
        let content_preview = manifest_content
            .map(|c| c.chars().take(500).collect::<String>())
            .unwrap_or_default();

        let prompt = format!(
            r#"Analyze this build manifest to identify the programming language. Respond with JSON only.

File: {}
Content:
{}

Response format:
{{
  "name": "LanguageName",
  "file_extensions": [".ext"],
  "package_managers": ["manager1"],
  "confidence": 0.95
}}
"#,
            manifest_name, content_preview
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.llm_client.chat(request))
        })
        .ok()?;

        let info: LanguageInfo = serde_json::from_str(&response.content).ok()?;

        if info.confidence < 0.5 {
            return None;
        }

        *self.detected_info.lock().unwrap() = Some(info.clone());

        Some(DetectionResult {
            build_system: crate::stack::BuildSystemId::Custom(
                info.package_managers
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
            confidence: info.confidence as f64,
        })
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &[]
    }

    fn parse_dependencies(
        &self,
        _manifest_content: &str,
        _all_internal_paths: &[std::path::PathBuf],
    ) -> DependencyInfo {
        DependencyInfo {
            internal_deps: vec![],
            external_deps: vec![],
            detected_by: DetectionMethod::LLM,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{MockLLMClient, MockResponse};

    #[test]
    fn test_llm_language_id_default() {
        let client = Arc::new(MockLLMClient::new());
        let language = LLMLanguage::new(client);
        assert_eq!(language.id(), LanguageId::Custom("Unknown".to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_llm_language_detect_success() {
        let info = LanguageInfo {
            name: "Zig".to_string(),
            file_extensions: vec![".zig".to_string()],
            package_managers: vec!["zig".to_string()],
            confidence: 0.9,
        };

        let json = serde_json::to_string(&info).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let language = LLMLanguage::new(client);
        let result = language.detect("build.zig", Some("const std = @import(\"std\");"));

        assert!(result.is_some());
        assert_eq!(language.id(), LanguageId::Custom("Zig".to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_confidence_validation() {
        let low_confidence = LanguageInfo {
            name: "Unknown".to_string(),
            file_extensions: vec![],
            package_managers: vec![],
            confidence: 0.3,
        };

        let json = serde_json::to_string(&low_confidence).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let language = LLMLanguage::new(client);
        let result = language.detect("unknown.txt", Some("content"));

        assert!(result.is_none());
    }
}
