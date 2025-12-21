use super::client::LLMClient;
use super::error::BackendError;
use super::selector::{select_llm_client, SelectedClient};
use super::types::{LLMRequest, LLMResponse};
use crate::config::AipackConfig;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tracing::debug;

pub struct LazyLLMClient {
    client: Arc<Mutex<Option<Arc<dyn LLMClient>>>>,
    config: AipackConfig,
    interactive: bool,
}

impl LazyLLMClient {
    pub fn new(config: AipackConfig, interactive: bool) -> Self {
        debug!("Creating LazyLLMClient - actual client selection deferred until first chat() call");
        Self {
            client: Arc::new(Mutex::new(None)),
            config,
            interactive,
        }
    }

    async fn ensure_initialized(&self) -> Result<Arc<dyn LLMClient>, BackendError> {
        // Check if already initialized (fast path)
        {
            let lock = self.client.lock().unwrap();
            if let Some(client) = lock.as_ref() {
                return Ok(client.clone());
            }
        }

        // Not initialized - need to select client
        debug!("Lazy initialization triggered - selecting LLM client now");
        let selected: SelectedClient = select_llm_client(&self.config, self.interactive)
            .await
            .map_err(|e| BackendError::Other {
                message: format!("Failed to initialize LLM client: {}", e),
            })?;

        debug!("LLM client selected: {}", selected.description);

        // Store the selected client
        {
            let mut lock = self.client.lock().unwrap();
            *lock = Some(selected.client.clone());
        }

        Ok(selected.client)
    }
}

#[async_trait]
impl LLMClient for LazyLLMClient {
    async fn chat(&self, request: LLMRequest) -> Result<LLMResponse, BackendError> {
        let client = self.ensure_initialized().await?;
        client.chat(request).await
    }

    fn name(&self) -> &str {
        "LazyLLMClient"
    }

    fn model_info(&self) -> Option<String> {
        let lock = self.client.lock().unwrap();
        lock.as_ref().and_then(|c| c.model_info())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lazy_client_name() {
        let config = AipackConfig::default();
        let client = LazyLLMClient::new(config, false);

        assert_eq!(client.name(), "LazyLLMClient");
    }

    #[tokio::test]
    async fn test_lazy_client_model_info_before_init() {
        let config = AipackConfig::default();
        let client = LazyLLMClient::new(config, false);

        // Before initialization, model_info should return None
        assert!(client.model_info().is_none());
    }
}
