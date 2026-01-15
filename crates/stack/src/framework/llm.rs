use super::{DependencyPattern, Framework, FrameworkConfig};
use crate::{language::Dependency, BuildTemplate, FrameworkId};
use peelbox_llm::{ChatMessage, LLMClient, LLMRequest};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrameworkInfo {
    name: String,
    language: String,
    dependency_patterns: Vec<String>,
    build_systems: Vec<String>,
    default_ports: Vec<u16>,
    health_endpoints: Vec<String>,
    confidence: f32,
}

pub struct LLMFramework {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<FrameworkInfo>>>,
}

impl LLMFramework {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            detected_info: Arc::new(Mutex::new(None)),
        }
    }

    pub fn detect_from_dependencies(&self, dependencies: &[Dependency]) -> bool {
        if dependencies.is_empty() {
            return false;
        }

        let deps_list = dependencies
            .iter()
            .take(20)
            .map(|d| {
                format!(
                    "{}{}",
                    d.name,
                    d.version
                        .as_ref()
                        .map(|v| format!("@{}", v))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        let prompt = format!(
            r#"Analyze these dependencies to identify the web framework. Respond with JSON ONLY.

Dependencies: {}

Response format ONLY:
{{
  "name": "FrameworkName",
  "language": "LanguageName",
  "dependency_patterns": ["pattern1", "pattern2"],
  "build_systems": ["npm", "yarn"],
  "default_ports": [3000, 8080],
  "health_endpoints": ["/health", "/ready"],
  "confidence": 0.95
}}
"#,
            deps_list
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.llm_client.chat(request))
        }) {
            Ok(resp) => resp,
            Err(_) => return false,
        };

        let info: FrameworkInfo = match serde_json::from_str(&response.content) {
            Ok(i) => i,
            Err(_) => return false,
        };

        if info.confidence < 0.5 {
            return false;
        }

        *self.detected_info.lock().unwrap() = Some(info);
        true
    }
}

impl Framework for LLMFramework {
    fn id(&self) -> FrameworkId {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| FrameworkId::Custom(info.name.clone()))
            .unwrap_or_else(|| FrameworkId::Custom("Unknown".to_string()))
    }

    fn compatible_languages(&self) -> Vec<String> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| vec![info.language.clone()])
            .unwrap_or_default()
    }

    fn compatible_build_systems(&self) -> Vec<String> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| info.build_systems.clone())
            .unwrap_or_default()
    }

    fn dependency_patterns(&self) -> Vec<DependencyPattern> {
        vec![]
    }

    fn default_ports(&self) -> Vec<u16> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| info.default_ports.clone())
            .unwrap_or_default()
    }

    fn health_endpoints(&self, _files: &[std::path::PathBuf]) -> Vec<String> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| info.health_endpoints.clone())
            .unwrap_or_default()
    }

    fn parse_config(&self, _file_path: &Path, _content: &str) -> Option<FrameworkConfig> {
        None
    }

    fn customize_build_template(&self, template: BuildTemplate) -> BuildTemplate {
        template
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peelbox_llm::{MockLLMClient, MockResponse};

    #[test]
    fn test_llm_framework_id_default() {
        let client = Arc::new(MockLLMClient::new());
        let framework = LLMFramework::new(client);
        assert_eq!(framework.id(), FrameworkId::Custom("Unknown".to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_llm_framework_detect_success() {
        let info = FrameworkInfo {
            name: "Remix".to_string(),
            language: "JavaScript".to_string(),
            dependency_patterns: vec!["@remix-run/react".to_string()],
            build_systems: vec!["npm".to_string(), "yarn".to_string()],
            default_ports: vec![3000],
            health_endpoints: vec!["/health".to_string()],
            confidence: 0.9,
        };

        let json = serde_json::to_string(&info).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let framework = LLMFramework::new(client);
        let deps = vec![Dependency {
            name: "@remix-run/react".to_string(),
            version: Some("1.0.0".to_string()),
            is_internal: false,
        }];

        let result = framework.detect_from_dependencies(&deps);

        assert!(result);
        assert_eq!(framework.id(), FrameworkId::Custom("Remix".to_string()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_confidence_validation() {
        let low_confidence = FrameworkInfo {
            name: "Unknown".to_string(),
            language: "Unknown".to_string(),
            dependency_patterns: vec![],
            build_systems: vec![],
            default_ports: vec![],
            health_endpoints: vec![],
            confidence: 0.1,
        };

        let json = serde_json::to_string(&low_confidence).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let framework = LLMFramework::new(client);
        let deps = vec![Dependency {
            name: "unknown".to_string(),
            version: None,
            is_internal: false,
        }];

        let result = framework.detect_from_dependencies(&deps);

        assert!(!result);
    }

    #[test]
    fn test_empty_dependencies() {
        let client = Arc::new(MockLLMClient::new());
        let framework = LLMFramework::new(client);
        let result = framework.detect_from_dependencies(&[]);

        assert!(!result);
    }
}
