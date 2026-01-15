use super::{HealthCheck, Runtime, RuntimeConfig};
use crate::framework::Framework;
use peelbox_llm::{ChatMessage, LLMClient, LLMRequest};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuntimeInfo {
    name: String,
    base_images: Vec<String>,
    system_packages: Vec<String>,
    start_command: String,
    confidence: f32,
}

pub struct LLMRuntime {
    llm_client: Option<Arc<dyn LLMClient>>,
    detected_info: Arc<Mutex<Option<RuntimeInfo>>>,
}

impl LLMRuntime {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client: Some(llm_client),
            detected_info: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for LLMRuntime {
    fn default() -> Self {
        Self {
            llm_client: None,
            detected_info: Arc::new(Mutex::new(None)),
        }
    }
}

impl Runtime for LLMRuntime {
    fn name(&self) -> &str {
        "LLM"
    }

    fn try_extract(
        &self,
        files: &[PathBuf],
        framework: Option<&dyn Framework>,
    ) -> Option<RuntimeConfig> {
        let port = framework.and_then(|f| f.default_ports().first().copied());
        let health = framework.and_then(|f| {
            f.health_endpoints(&[]).first().map(|endpoint| HealthCheck {
                endpoint: endpoint.to_string(),
            })
        });

        if files.is_empty() || self.llm_client.is_none() {
            return Some(RuntimeConfig {
                entrypoint: None,
                port,
                env_vars: vec![],
                health,
                native_deps: vec![],
            });
        }

        let llm_client = self.llm_client.as_ref().unwrap();

        let framework_info = framework.map(|f| format!("Framework: {:?}", f.id()));
        let files_list = files
            .iter()
            .take(5)
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let prompt = format!(
            r#"Analyze the runtime configuration for this application. Respond with JSON ONLY.

Files: {}
{}

Response format ONLY:
{{
  "name": "RuntimeName",
  "base_images": ["image:tag"],
  "system_packages": ["pkg1", "pkg2"],
  "start_command": "command",
  "confidence": 0.95
}}
"#,
            files_list,
            framework_info.unwrap_or_default()
        );

        let request = LLMRequest::new(vec![ChatMessage::user(prompt)]);
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(llm_client.chat(request))
        })
        .ok()?;

        let info: RuntimeInfo = serde_json::from_str(&response.content).ok()?;

        if info.confidence < 0.5 {
            return None;
        }

        *self.detected_info.lock().unwrap() = Some(info.clone());

        Some(RuntimeConfig {
            entrypoint: None,
            port,
            env_vars: vec![],
            health,
            native_deps: info.system_packages,
        })
    }

    fn runtime_base_image(&self, _version: Option<&str>) -> String {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|info| info.base_images.first().cloned())
            .unwrap_or_else(|| "alpine:latest".to_string())
    }

    fn required_packages(&self) -> Vec<String> {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| info.system_packages.clone())
            .unwrap_or_default()
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        self.detected_info
            .lock()
            .unwrap()
            .as_ref()
            .map(|info| info.start_command.clone())
            .unwrap_or_else(|| format!("./{}", entrypoint.display()))
    }

    fn runtime_packages(
        &self,
        _wolfi_index: &peelbox_wolfi::WolfiPackageIndex,
        _service_path: &Path,
        _manifest_content: Option<&str>,
    ) -> Vec<String> {
        vec!["glibc".to_string(), "ca-certificates".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peelbox_llm::MockLLMClient;

    #[test]
    fn test_llm_runtime_name() {
        let client = Arc::new(MockLLMClient::new());
        let runtime = LLMRuntime::new(client);
        assert_eq!(runtime.name(), "LLM");
    }

    #[test]
    fn test_llm_runtime_base_image_default() {
        let client = Arc::new(MockLLMClient::new());
        let runtime = LLMRuntime::new(client);
        assert_eq!(runtime.runtime_base_image(None), "alpine:latest");
    }

    #[test]
    fn test_llm_required_packages() {
        let client = Arc::new(MockLLMClient::new());
        let runtime = LLMRuntime::new(client);
        let packages: Vec<String> = vec![];
        assert_eq!(runtime.required_packages(), packages);
    }

    #[test]
    fn test_llm_start_command() {
        let client = Arc::new(MockLLMClient::new());
        let runtime = LLMRuntime::new(client);
        let entrypoint = Path::new("unknown");
        assert_eq!(runtime.start_command(entrypoint), "./unknown");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_confidence_validation() {
        use peelbox_llm::MockResponse;

        let low_confidence = RuntimeInfo {
            name: "TestRuntime".to_string(),
            base_images: vec!["test:latest".to_string()],
            system_packages: vec![],
            start_command: "./app".to_string(),
            confidence: 0.3,
        };

        let json = serde_json::to_string(&low_confidence).unwrap();
        let client = Arc::new(MockLLMClient::new());
        client.add_response(MockResponse::text(json));

        let runtime = LLMRuntime::new(client);
        let result = runtime.try_extract(&[PathBuf::from("test.txt")], None);

        assert!(result.is_none());
    }
}
