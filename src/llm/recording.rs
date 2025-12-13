//! LLM request-response recording for deterministic testing

use crate::ai::error::BackendError;
use crate::llm::{ChatMessage, LLMClient, LLMRequest, LLMResponse};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Recording mode for LLM interactions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingMode {
    /// Record new exchanges and save to disk
    Record,
    /// Replay from recorded exchanges, fail if not found
    Replay,
    /// Replay if recording exists, otherwise record
    Auto,
}

impl RecordingMode {
    /// Parse from string
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "record" => Ok(RecordingMode::Record),
            "replay" => Ok(RecordingMode::Replay),
            "auto" => Ok(RecordingMode::Auto),
            _ => anyhow::bail!("Invalid recording mode: {}", s),
        }
    }

    /// Get from environment variable with default
    pub fn from_env(default: RecordingMode) -> RecordingMode {
        std::env::var("AIPACK_RECORDING_MODE")
            .ok()
            .and_then(|s| Self::parse(&s).ok())
            .unwrap_or(default)
    }
}

/// A recorded request-response exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedExchange {
    /// Canonical hash of the request (MD5)
    pub request_hash: String,
    /// The original request
    pub request: RecordedRequest,
    /// The recorded response
    pub response: LLMResponse,
    /// All intermediate responses during tool calling loop (for analysis)
    pub intermediate_responses: Vec<LLMResponse>,
    /// Timestamp when recorded (ISO 8601)
    pub recorded_at: String,
}

/// Simplified request for hashing and storage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordedRequest {
    /// Messages in the conversation
    pub messages: Vec<ChatMessage>,
    /// Available tools
    pub tools: Vec<serde_json::Value>,
    /// Model name
    pub model: Option<String>,
}

impl RecordedRequest {
    /// Create from LLMRequest
    pub fn from_llm_request(req: &LLMRequest) -> Self {
        Self {
            messages: req.messages.clone(),
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
            model: None,
        }
    }

    /// Compute canonical hash (MD5 of JSON)
    pub fn canonical_hash(&self) -> String {
        let canonical_json = serde_json::to_string(self).expect("Failed to serialize request");
        format!("{:x}", md5::compute(canonical_json.as_bytes()))
    }
}

/// LLM client that records or replays interactions
pub struct RecordingLLMClient {
    /// Underlying LLM client
    inner: Arc<dyn LLMClient>,
    /// Recording mode
    mode: RecordingMode,
    /// Directory where recordings are stored
    recordings_dir: PathBuf,
    /// In-memory cache of loaded recordings
    cache: HashMap<String, LLMResponse>,
    /// Intermediate responses captured during tool-calling loop
    intermediate_responses: std::sync::Mutex<Vec<LLMResponse>>,
}

impl RecordingLLMClient {
    /// Create a new recording client
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
            intermediate_responses: std::sync::Mutex::new(Vec::new()),
        })
    }

    /// Create with defaults from environment
    pub fn from_env(inner: Arc<dyn LLMClient>) -> Result<Self> {
        let mode = RecordingMode::from_env(RecordingMode::Auto);
        let recordings_dir = std::env::var("AIPACK_RECORDINGS_DIR")
            .unwrap_or_else(|_| "tests/recordings".to_string())
            .into();

        Self::new(inner, mode, recordings_dir)
    }

    /// Get path to recording file for a request hash
    fn recording_path(&self, request_hash: &str) -> PathBuf {
        self.recordings_dir.join(format!("{}.json", request_hash))
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

        // Collect all intermediate responses
        let intermediates = self.intermediate_responses.lock().unwrap().clone();

        let exchange = RecordedExchange {
            request_hash: request_hash.clone(),
            request: request.clone(),
            response: response.clone(),
            intermediate_responses: intermediates,
            recorded_at: chrono::Utc::now().to_rfc3339(),
        };

        // Save JSON recording
        let path = self.recording_path(&request_hash);
        let contents =
            serde_json::to_string_pretty(&exchange).context("Failed to serialize recording")?;
        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write recording: {}", path.display()))?;

        // Clear intermediate responses for next recording
        self.intermediate_responses.lock().unwrap().clear();

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
        let recorded_request = RecordedRequest::from_llm_request(&request);
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

                Err(BackendError::Other {
                    message: format!(
                        "No recording found for request hash: {} (mode: Replay)",
                        request_hash
                    ),
                })
            }
            RecordingMode::Record => {
                // Always call the underlying client
                let response = self.inner.chat(request).await?;

                // Store intermediate response for tool-calling loop analysis
                self.intermediate_responses
                    .lock()
                    .unwrap()
                    .push(response.clone());

                // Only save recording on final submission (detect submit_detection tool call)
                let is_final = response
                    .tool_calls
                    .iter()
                    .any(|call| call.name == "submit_detection");

                if is_final {
                    self.save_recording(&recorded_request, &response)
                        .map_err(|e| BackendError::Other {
                            message: format!("Failed to save recording: {}", e),
                        })?;
                }

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

                // Store intermediate response for tool-calling loop analysis
                self.intermediate_responses
                    .lock()
                    .unwrap()
                    .push(response.clone());

                // Only save recording on final submission (detect submit_detection tool call)
                let is_final = response
                    .tool_calls
                    .iter()
                    .any(|call| call.name == "submit_detection");

                if is_final {
                    self.save_recording(&recorded_request, &response)
                        .map_err(|e| BackendError::Other {
                            message: format!("Failed to save recording: {}", e),
                        })?;
                }

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
    use crate::llm::{MockLLMClient, MockResponse};

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
        use crate::llm::ToolCall;

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

        // Check that recording was saved
        let recorded_request = RecordedRequest {
            messages: vec![ChatMessage::user("Test")],
            tools: vec![],
            model: None,
        };
        let hash = recorded_request.canonical_hash();
        let recording_path = recordings_dir.join(format!("{}.json", hash));

        assert!(recording_path.exists());
    }

    #[tokio::test]
    async fn test_recording_client_replay_mode() {
        use crate::llm::ToolCall;

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
        use crate::llm::ToolCall;

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
