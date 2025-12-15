use crate::llm::LLMClient;
use anyhow::{Context, Result};

pub async fn query_llm<T: serde::de::DeserializeOwned>(
    llm_client: &dyn LLMClient,
    prompt: String,
    max_tokens: u32,
    context_msg: &str,
) -> Result<T> {
    let request = crate::llm::LLMRequest::new(vec![crate::llm::ChatMessage::user(prompt)])
        .with_temperature(0.1)
        .with_max_tokens(max_tokens);

    let response = llm_client
        .chat(request)
        .await
        .with_context(|| format!("Failed to call LLM for {}", context_msg))?;

    serde_json::from_str(&response.content)
        .with_context(|| format!("Failed to parse {} response", context_msg))
}
