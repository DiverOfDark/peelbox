//! LLM client selection with automatic fallback chain
//!
//! This module provides automatic LLM client selection based on available
//! providers and configuration. The fallback chain is:
//!
//! 1. Configured provider (via env/CLI) if API key is available
//! 2. Ollama if running locally
//! 3. Embedded LLM for zero-config local inference

use crate::ai::genai_backend::{GenAIBackend, Provider};
use crate::config::AipackConfig;
use crate::llm::{EmbeddedClient, LLMClient};
use anyhow::Result;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Result of LLM client selection
pub struct SelectedClient {
    /// The selected LLM client
    pub client: Arc<dyn LLMClient>,
    /// The provider that was selected
    pub provider: Provider,
    /// Human-readable description of the selection
    pub description: String,
}

/// Select the best available LLM client based on configuration and availability
///
/// Fallback chain:
/// 1. If a provider is explicitly configured with valid credentials, use it
/// 2. Try Ollama if available locally
/// 3. Fall back to embedded LLM for zero-config local inference
pub async fn select_llm_client(config: &AipackConfig, interactive: bool) -> Result<SelectedClient> {
    // First, try the explicitly configured provider
    if let Some(selected) = try_configured_provider(config).await {
        return Ok(selected);
    }

    // Try Ollama as fallback
    if let Some(selected) = try_ollama(config).await {
        return Ok(selected);
    }

    // Try embedded LLM as last resort
    if let Some(selected) = try_embedded(interactive).await {
        return Ok(selected);
    }

    // No LLM available
    Err(anyhow::anyhow!(
        "No LLM backend available. Please either:\n\
         - Set an API key (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)\n\
         - Start Ollama locally (ollama serve)\n\
         - Ensure sufficient RAM for embedded LLM (minimum 3GB available)"
    ))
}

/// Try to use the explicitly configured provider (only for cloud providers with credentials)
///
/// This function only attempts cloud providers that have API keys configured.
/// Ollama is handled separately by `try_ollama` to properly check availability.
async fn try_configured_provider(config: &AipackConfig) -> Option<SelectedClient> {
    let provider = config.provider;

    // Skip Ollama here - it's handled by try_ollama with proper availability check
    if provider == Provider::Ollama {
        debug!("Skipping Ollama in configured provider check - will check availability separately");
        return None;
    }

    // Check if API key is available for cloud providers
    if !provider_has_credentials(provider) {
        debug!("Skipping {} - no credentials available", provider);
        return None;
    }

    match GenAIBackend::new(provider, config.model.clone()).await {
        Ok(backend) => {
            info!("Using configured provider: {} ({})", provider, config.model);
            Some(SelectedClient {
                client: backend.into_llm_client(),
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

/// Try to use Ollama as a fallback
async fn try_ollama(config: &AipackConfig) -> Option<SelectedClient> {
    // Check if Ollama is running
    if !is_ollama_available().await {
        debug!("Ollama not available");
        return None;
    }

    let model = if config.provider == Provider::Ollama {
        config.model.clone()
    } else {
        // Use a sensible default model for Ollama
        "qwen2.5-coder:7b".to_string()
    };

    match GenAIBackend::new(Provider::Ollama, model.clone()).await {
        Ok(backend) => {
            info!("Using Ollama with model: {}", model);
            Some(SelectedClient {
                client: backend.into_llm_client(),
                provider: Provider::Ollama,
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
                provider: Provider::Ollama, // Closest match for embedded
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
fn provider_has_credentials(provider: Provider) -> bool {
    match provider {
        Provider::Ollama => true, // No credentials needed
        Provider::OpenAI => std::env::var("OPENAI_API_KEY").is_ok(),
        Provider::Claude => std::env::var("ANTHROPIC_API_KEY").is_ok(),
        Provider::Gemini => std::env::var("GOOGLE_API_KEY").is_ok(),
        Provider::Grok => std::env::var("XAI_API_KEY").is_ok(),
        Provider::Groq => std::env::var("GROQ_API_KEY").is_ok(),
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
        assert!(provider_has_credentials(Provider::Ollama));

        // Cloud providers depend on environment variables
        // These tests just verify the function doesn't panic
        let _ = provider_has_credentials(Provider::OpenAI);
        let _ = provider_has_credentials(Provider::Claude);
        let _ = provider_has_credentials(Provider::Gemini);
    }
}
