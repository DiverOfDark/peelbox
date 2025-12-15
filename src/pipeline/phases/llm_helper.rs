use crate::heuristics::HeuristicLogger;
use crate::llm::LLMClient;
use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Instant;

pub async fn query_llm_with_logging<T: serde::de::DeserializeOwned + serde::Serialize>(
    llm_client: &dyn LLMClient,
    prompt: String,
    max_tokens: u32,
    phase: &str,
    logger: &Arc<HeuristicLogger>,
) -> Result<T> {
    let start = Instant::now();

    let request = crate::llm::LLMRequest::new(vec![crate::llm::ChatMessage::user(prompt.clone())])
        .with_temperature(0.1)
        .with_max_tokens(max_tokens);

    let response = llm_client
        .chat(request.clone())
        .await
        .with_context(|| format!("Failed to call LLM for {}", phase))?;

    let latency_ms = start.elapsed().as_millis() as u64;

    let parsed: T = serde_json::from_str(&response.content)
        .with_context(|| format!("Failed to parse {} response", phase))?;

    logger.log_phase(phase, &request, &response, latency_ms);

    Ok(parsed)
}
