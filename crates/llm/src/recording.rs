use crate::{BackendError, ChatMessage, LLMClient, LLMRequest, LLMResponse, TestContext};
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingMode {
    Record,
    Replay,
    Auto,
}

impl RecordingMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "record" => Ok(RecordingMode::Record),
            "replay" => Ok(RecordingMode::Replay),
            "auto" => Ok(RecordingMode::Auto),
            _ => anyhow::bail!("Invalid recording mode: {}", s),
        }
    }

    pub fn from_env(default: RecordingMode) -> RecordingMode {
        std::env::var("PEELBOX_RECORDING_MODE")
            .ok()
            .and_then(|s| Self::parse(&s).ok())
            .unwrap_or(default)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedExchange {
    pub request_hash: String,
    pub request: RecordedRequest,
    pub response: LLMResponse,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordedRequest {
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<serde_json::Value>,
    pub model: Option<String>,
}

impl RecordedRequest {
    pub fn from_llm_request(req: &LLMRequest, model: Option<String>) -> Self {
        let messages = req
            .messages
            .iter()
            .map(|msg| {
                let mut m = msg.clone();
                m.content = Self::normalize_content(&m.content);
                m
            })
            .collect();

        Self {
            messages,
            tools: req
                .tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    })
                })
                .collect(),
            model,
        }
    }

    fn normalize_content(content: &str) -> String {
        let mut normalized = content.to_string();

        // Replace current working directory with a placeholder to ensure determinism across different environments
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_str = cwd.to_string_lossy().to_string();
            if !cwd_str.is_empty() && cwd_str != "/" {
                normalized = normalized.replace(&cwd_str, "[REPO_ROOT]");
            }
        }

        // Normalize /tmp paths (e.g. /tmp/.tmpXXXXXX)
        static TEMP_PATH_REGEX: OnceLock<Regex> = OnceLock::new();
        let temp_re = TEMP_PATH_REGEX.get_or_init(|| {
            // Match /tmp/ followed by path characters
            Regex::new(r"/tmp/[\w\-\./]+").expect("Invalid temp path regex")
        });
        normalized = temp_re.replace_all(&normalized, "[TEMP_DIR]").to_string();

        // Normalize UUIDs
        static UUID_REGEX: OnceLock<Regex> = OnceLock::new();
        let uuid_re = UUID_REGEX.get_or_init(|| {
            Regex::new(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
                .expect("Invalid UUID regex")
        });
        normalized = uuid_re.replace_all(&normalized, "[UUID]").to_string();

        normalized
    }

    pub fn canonical_hash(&self) -> String {
        let canonical_json = serde_json::to_string(self).expect("Failed to serialize request");
        format!("{:x}", md5::compute(canonical_json.as_bytes()))
    }
}

pub struct RecordingLLMClient {
    inner: Arc<dyn LLMClient>,
    mode: RecordingMode,
    recordings_dir: PathBuf,
    cache: HashMap<String, LLMResponse>,
}

impl RecordingLLMClient {
    pub fn new(
        inner: Arc<dyn LLMClient>,
        mode: RecordingMode,
        recordings_dir: PathBuf,
    ) -> Result<Self> {
        std::fs::create_dir_all(&recordings_dir)
            .context("Failed to create recordings directory")?;

        Ok(Self {
            inner,
            mode,
            recordings_dir,
            cache: HashMap::new(),
        })
    }

    /// Create with defaults from environment
    pub fn from_env(inner: Arc<dyn LLMClient>) -> Result<Self> {
        let mode = RecordingMode::from_env(RecordingMode::Auto);
        let recordings_dir = std::env::var("PEELBOX_RECORDINGS_DIR")
            .unwrap_or_else(|_| "tests/recordings".to_string())
            .into();

        Self::new(inner, mode, recordings_dir)
    }

    /// Get path to recording file for a request hash
    fn recording_path(&self, request_hash: &str) -> PathBuf {
        if let Some(test_name) = TestContext::current_test_name() {
            self.recordings_dir
                .join(format!("{}__{}.json", test_name, request_hash))
        } else {
            self.recordings_dir.join(format!("{}.json", request_hash))
        }
    }

    /// Load recording from disk
    fn load_recording(&self, request_hash: &str) -> Result<Option<LLMResponse>> {
        let path = self.recording_path(request_hash);
        if !path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read recording: {}", path.display()))?;

        let exchange: RecordedExchange = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse recording: {}", path.display()))?;

        Ok(Some(exchange.response))
    }

    /// Save recording to disk
    fn save_recording(&self, request: &RecordedRequest, response: &LLMResponse) -> Result<()> {
        let request_hash = request.canonical_hash();

        let exchange = RecordedExchange {
            request_hash: request_hash.clone(),
            request: request.clone(),
            response: response.clone(),
            recorded_at: chrono::Utc::now().to_rfc3339(),
        };

        // Save JSON recording
        let path = self.recording_path(&request_hash);
        let contents =
            serde_json::to_string_pretty(&exchange).context("Failed to serialize recording")?;
        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write recording: {}", path.display()))?;

        Ok(())
    }

    /// Load all recordings into cache
    pub fn preload_cache(&mut self) -> Result<()> {
        if !self.recordings_dir.exists() {
            return Ok(());
        }

        for entry in std::fs::read_dir(&self.recordings_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let contents = std::fs::read_to_string(&path)?;
            let exchange: RecordedExchange = serde_json::from_str(&contents)?;

            self.cache
                .insert(exchange.request_hash.clone(), exchange.response);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl LLMClient for RecordingLLMClient {
    async fn chat(&self, request: LLMRequest) -> Result<LLMResponse, BackendError> {
        let model_info = self.inner.model_info();
        let recorded_request = RecordedRequest::from_llm_request(&request, model_info);
        let request_hash = recorded_request.canonical_hash();

        match self.mode {
            RecordingMode::Replay => {
                // Check cache first
                if let Some(response) = self.cache.get(&request_hash) {
                    return Ok(response.clone());
                }

                // Try loading from disk
                if let Some(response) =
                    self.load_recording(&request_hash)
                        .map_err(|e| BackendError::Other {
                            message: format!("Failed to load recording: {}", e),
                        })?
                {
                    return Ok(response);
                }

                // Dump the request to help debugging
                let dump_path = self.recordings_dir.join(format!(
                    "MISSING_{}.json",
                    TestContext::current_test_name().unwrap_or_else(|| "unknown_test".to_string())
                ));
                let _ = std::fs::write(
                    &dump_path,
                    serde_json::to_string_pretty(&recorded_request).unwrap_or_default(),
                );

                let dump_path = self.recordings_dir.join(format!(
                    "MISSING_{}.json",
                    TestContext::current_test_name().unwrap_or_else(|| "unknown_test".to_string())
                ));
                let _ = std::fs::write(
                    &dump_path,
                    serde_json::to_string_pretty(&recorded_request).unwrap_or_default(),
                );

                Err(BackendError::Other {
                    message: format!(
                        "No recording found for request hash: {} (mode: Replay). Dumped request to {}",
                        request_hash,
                        dump_path.display()
                    ),
                })
            }
            RecordingMode::Record => {
                // Always call the underlying client
                let response = self.inner.chat(request).await?;

                // Always save recording for this specific request
                self.save_recording(&recorded_request, &response)
                    .map_err(|e| BackendError::Other {
                        message: format!("Failed to save recording: {}", e),
                    })?;

                Ok(response)
            }
            RecordingMode::Auto => {
                // Check cache first
                if let Some(response) = self.cache.get(&request_hash) {
                    return Ok(response.clone());
                }

                // Try loading from disk
                if let Some(response) =
                    self.load_recording(&request_hash)
                        .map_err(|e| BackendError::Other {
                            message: format!("Failed to load recording: {}", e),
                        })?
                {
                    return Ok(response);
                }

                // No recording found, call underlying client and record
                let response = self.inner.chat(request).await?;

                // Always save recording for this specific request
                self.save_recording(&recorded_request, &response)
                    .map_err(|e| BackendError::Other {
                        message: format!("Failed to save recording: {}", e),
                    })?;

                Ok(response)
            }
        }
    }

    fn name(&self) -> &str {
        "RecordingLLMClient"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MockLLMClient, MockResponse};

    #[test]
    fn test_recording_mode_parse() {
        assert_eq!(
            RecordingMode::parse("record").unwrap(),
            RecordingMode::Record
        );
        assert_eq!(
            RecordingMode::parse("replay").unwrap(),
            RecordingMode::Replay
        );
        assert_eq!(RecordingMode::parse("auto").unwrap(), RecordingMode::Auto);
        assert_eq!(
            RecordingMode::parse("RECORD").unwrap(),
            RecordingMode::Record
        );
        assert!(RecordingMode::parse("invalid").is_err());
    }

    #[test]
    fn test_recorded_request_hash() {
        let req1 = RecordedRequest {
            messages: vec![ChatMessage::user("Hello")],
            tools: vec![],
            model: None,
        };

        let req2 = RecordedRequest {
            messages: vec![ChatMessage::user("Hello")],
            tools: vec![],
            model: None,
        };

        let req3 = RecordedRequest {
            messages: vec![ChatMessage::user("Different")],
            tools: vec![],
            model: None,
        };

        assert_eq!(req1.canonical_hash(), req2.canonical_hash());
        assert_ne!(req1.canonical_hash(), req3.canonical_hash());
    }

    #[tokio::test]
    async fn test_recording_client_record_mode() {
        use crate::ToolCall;

        let temp_dir = tempfile::tempdir().unwrap();
        let recordings_dir = temp_dir.path().to_path_buf();

        let mock_client = Arc::new(MockLLMClient::new());
        // Response with submit_detection tool call to trigger recording
        mock_client.add_response(MockResponse::with_tool_calls(
            "Submitting detection".to_string(),
            vec![ToolCall {
                call_id: "1".to_string(),
                name: "submit_detection".to_string(),
                arguments: serde_json::json!({}),
            }],
        ));

        let recording_client =
            RecordingLLMClient::new(mock_client, RecordingMode::Record, recordings_dir.clone())
                .unwrap();

        let request = LLMRequest::new(vec![ChatMessage::user("Test")])
            .with_max_tokens(100)
            .with_temperature(0.7);

        let response = recording_client.chat(request).await.unwrap();
        assert_eq!(response.content, "Submitting detection");

        // Check that recording was saved with test name prefix
        let recorded_request = RecordedRequest {
            messages: vec![ChatMessage::user("Test")],
            tools: vec![],
            model: Some("mock-model".to_string()),
        };
        let hash = recorded_request.canonical_hash();

        // Recording should include test name prefix
        let test_name = TestContext::current_test_name().expect("Should be in test context");
        let recording_path = recordings_dir.join(format!("{}__{}.json", test_name, hash));

        assert!(recording_path.exists());
    }

    #[tokio::test]
    async fn test_recording_client_replay_mode() {
        use crate::ToolCall;

        let temp_dir = tempfile::tempdir().unwrap();
        let recordings_dir = temp_dir.path().to_path_buf();

        // First, record
        let mock_client = Arc::new(MockLLMClient::new());
        mock_client.add_response(MockResponse::with_tool_calls(
            "Submitting detection".to_string(),
            vec![ToolCall {
                call_id: "1".to_string(),
                name: "submit_detection".to_string(),
                arguments: serde_json::json!({}),
            }],
        ));

        let recording_client =
            RecordingLLMClient::new(mock_client, RecordingMode::Record, recordings_dir.clone())
                .unwrap();

        let request = LLMRequest::new(vec![ChatMessage::user("Test")])
            .with_max_tokens(100)
            .with_temperature(0.7);

        recording_client.chat(request.clone()).await.unwrap();

        // Now replay (mock client won't be called)
        let mock_client = Arc::new(MockLLMClient::new());

        let replay_client =
            RecordingLLMClient::new(mock_client, RecordingMode::Replay, recordings_dir.clone())
                .unwrap();

        let response = replay_client.chat(request).await.unwrap();
        assert_eq!(response.content, "Submitting detection");
    }

    #[tokio::test]
    async fn test_recording_client_auto_mode() {
        use crate::ToolCall;

        let temp_dir = tempfile::tempdir().unwrap();
        let recordings_dir = temp_dir.path().to_path_buf();

        let mock_client = Arc::new(MockLLMClient::new());
        mock_client.add_response(MockResponse::with_tool_calls(
            "Submitting detection".to_string(),
            vec![ToolCall {
                call_id: "1".to_string(),
                name: "submit_detection".to_string(),
                arguments: serde_json::json!({}),
            }],
        ));

        let auto_client =
            RecordingLLMClient::new(mock_client, RecordingMode::Auto, recordings_dir.clone())
                .unwrap();

        let request = LLMRequest::new(vec![ChatMessage::user("Test")])
            .with_max_tokens(100)
            .with_temperature(0.7);

        // First call records
        let response1 = auto_client.chat(request.clone()).await.unwrap();
        assert_eq!(response1.content, "Submitting detection");

        // Second call replays (mock client won't be called again)
        let mock_client = Arc::new(MockLLMClient::new());
        let auto_client2 =
            RecordingLLMClient::new(mock_client, RecordingMode::Auto, recordings_dir.clone())
                .unwrap();

        let response2 = auto_client2.chat(request).await.unwrap();
        assert_eq!(response2.content, "Submitting detection");
    }
}
