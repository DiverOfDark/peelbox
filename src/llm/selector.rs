use crate::config::AipackConfig;
use crate::llm::{EmbeddedClient, GenAIClient, LLMClient};
use anyhow::Result;
use genai::adapter::AdapterKind;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub struct SelectedClient {
    pub client: Arc<dyn LLMClient>,
    pub provider: AdapterKind,
    pub description: String,
}

pub async fn select_llm_client(config: &AipackConfig, interactive: bool) -> Result<SelectedClient> {
    if let Some(selected) = try_configured_provider(config).await {
        return Ok(selected);
    }

    if let Some(selected) = try_ollama(config).await {
        return Ok(selected);
    }

    if let Some(selected) = try_embedded(interactive).await {
        return Ok(selected);
    }

    Err(anyhow::anyhow!(
        "No LLM backend available. Please either:\n\
         - Set an API key (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)\n\
         - Start Ollama locally (ollama serve)\n\
         - Ensure sufficient RAM for embedded LLM (minimum 3GB available)"
    ))
}

async fn try_configured_provider(config: &AipackConfig) -> Option<SelectedClient> {
    let provider = config.provider;

    if provider == AdapterKind::Ollama {
        debug!("Skipping Ollama in configured provider check - will check availability separately");
        return None;
    }

    if !provider_has_credentials(provider) {
        debug!("Skipping {} - no credentials available", provider);
        return None;
    }

    match GenAIClient::new(
        provider,
        config.model.clone(),
        std::time::Duration::from_secs(config.request_timeout_secs),
    )
    .await
    {
        Ok(client) => {
            info!("Using configured provider: {} ({})", provider, config.model);
            Some(SelectedClient {
                client: Arc::new(client),
                provider,
                description: format!("{} ({})", provider, config.model),
            })
        }
        Err(e) => {
            warn!("Failed to initialize {}: {}", provider, e);
            None
        }
    }
}

async fn try_ollama(config: &AipackConfig) -> Option<SelectedClient> {
    // Check if Ollama is running
    if !is_ollama_available().await {
        debug!("Ollama not available");
        return None;
    }

    let model = if config.provider == AdapterKind::Ollama {
        config.model.clone()
    } else {
        // Use a sensible default model for Ollama
        "qwen2.5-coder:7b".to_string()
    };

    match GenAIClient::new(
        AdapterKind::Ollama,
        model.clone(),
        std::time::Duration::from_secs(config.request_timeout_secs),
    )
    .await
    {
        Ok(client) => {
            info!("Using Ollama with model: {}", model);
            Some(SelectedClient {
                client: Arc::new(client),
                provider: AdapterKind::Ollama,
                description: format!("Ollama ({})", model),
            })
        }
        Err(e) => {
            warn!("Failed to initialize Ollama: {}", e);
            None
        }
    }
}

/// Try to use embedded LLM as last resort
async fn try_embedded(interactive: bool) -> Option<SelectedClient> {
    match EmbeddedClient::new(interactive).await {
        Ok(client) => {
            let model_info = client
                .model_info()
                .unwrap_or_else(|| "embedded".to_string());
            info!("Using embedded LLM: {}", model_info);
            Some(SelectedClient {
                client: Arc::new(client),
                provider: AdapterKind::Ollama, // Closest match for embedded
                description: format!("Embedded ({})", model_info),
            })
        }
        Err(e) => {
            warn!("Failed to initialize embedded LLM: {}", e);
            None
        }
    }
}

/// Check if provider has available credentials
fn provider_has_credentials(provider: AdapterKind) -> bool {
    match provider.default_key_env_name() {
        None => true, // No credentials needed (e.g., Ollama)
        Some(env_var) => std::env::var(env_var).is_ok(),
    }
}

/// Check if Ollama is running locally
async fn is_ollama_available() -> bool {
    let base_url =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let url = format!("{}/api/tags", base_url);

    match reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(resp) => {
            let available = resp.status().is_success();
            debug!("Ollama availability check: {}", available);
            available
        }
        Err(e) => {
            debug!("Ollama not available: {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_credentials_check() {
        // Ollama should always return true (no credentials needed)
        assert!(provider_has_credentials(AdapterKind::Ollama));

        // Cloud providers depend on environment variables
        // These tests just verify the function doesn't panic
        let _ = provider_has_credentials(AdapterKind::OpenAI);
        let _ = provider_has_credentials(AdapterKind::Anthropic);
        let _ = provider_has_credentials(AdapterKind::Gemini);
    }
}
