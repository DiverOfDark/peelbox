use super::{MonorepoOrchestrator, OrchestratorId};
use crate::llm::{ChatMessage, LLMClient, LLMRequest};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrchestratorInfo {
    name: String,
    config_files: Vec<String>,
    cache_dirs: Vec<String>,
    confidence: f32,
}

pub struct LLMOrchestrator {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<OrchestratorInfo>>>,
}

impl LLMOrchestrator {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            detected_info: Arc::new(Mutex::new(None)),
        }
    }
}

impl MonorepoOrchestrator for LLMOrchestrator {
    fn id(&self) -> OrchestratorId {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| OrchestratorId::Custom(info.name.clone()))
            .unwrap_or_else(|| OrchestratorId::Custom("Unknown".to_string()))
    }

    fn config_files(&self) -> Vec<String> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| info.config_files.clone())
            .unwrap_or_default()
    }

    fn detect(&self, config_file: &str, content: Option<&str>) -> bool {
        let content_preview = content
            .map(|c| c.chars().take(500).collect::<String>())
            .unwrap_or_default();

        let prompt = format!(
            r#"Analyze this configuration to identify the monorepo orchestrator. Respond with JSON only.

Config file: {}
Content:
{}

Response format:
{{
  "name": "OrchestratorName",
  "config_files": ["file1.json"],
  "cache_dirs": [".cache"],
  "confidence": 0.95
}}
"#,
            config_file, content_preview
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.llm_client.chat(request))
        }) {
            Ok(resp) => resp,
            Err(_) => return false,
        };

        let info: OrchestratorInfo = match serde_json::from_str(&response.content) {
            Ok(i) => i,
            Err(_) => return false,
        };

        if info.confidence < 0.5 {
            return false;
        }

        *self.detected_info.lock().unwrap() = Some(info);
        true
    }

    fn cache_dirs(&self) -> Vec<String> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| info.cache_dirs.clone())
            .unwrap_or_default()
    }

    fn name(&self) -> &'static str {
        "LLM"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{MockLLMClient, MockResponse};

    #[test]
    fn test_llm_orchestrator_id_default() {
        let client = Arc::new(MockLLMClient::new());
        let orchestrator = LLMOrchestrator::new(client);
        assert_eq!(
            orchestrator.id(),
            OrchestratorId::Custom("Unknown".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_llm_orchestrator_detect_success() {
        let info = OrchestratorInfo {
            name: "Rush".to_string(),
            config_files: vec!["rush.json".to_string()],
            cache_dirs: vec!["common/temp".to_string()],
            confidence: 0.9,
        };

        let json = serde_json::to_string(&info).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let orchestrator = LLMOrchestrator::new(client);
        let result = orchestrator.detect("rush.json", Some("{\"version\": 5}"));

        assert!(result);
        assert_eq!(
            orchestrator.id(),
            OrchestratorId::Custom("Rush".to_string())
        );
        assert_eq!(orchestrator.cache_dirs(), vec!["common/temp"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_confidence_validation() {
        let low_confidence = OrchestratorInfo {
            name: "Unknown".to_string(),
            config_files: vec![],
            cache_dirs: vec![],
            confidence: 0.25,
        };

        let json = serde_json::to_string(&low_confidence).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let orchestrator = LLMOrchestrator::new(client);
        let result = orchestrator.detect("unknown.txt", Some("content"));

        assert!(!result);
    }

    #[test]
    fn test_orchestrator_name() {
        let client = Arc::new(MockLLMClient::new());
        let orchestrator = LLMOrchestrator::new(client);
        assert_eq!(orchestrator.name(), "LLM");
    }
}
