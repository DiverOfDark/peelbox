use super::client::LLMClient;
use super::types::{ChatMessage, LLMRequest, LLMResponse, MessageRole, ToolCall, ToolDefinition};
use crate::ai::error::BackendError;
use async_trait::async_trait;
use genai::adapter::AdapterKind;
use genai::chat::{
    ChatMessage as GenAIChatMessage, ChatOptions, ChatRequest as GenAIChatRequest, MessageContent,
    Tool as GenAITool, ToolResponse,
};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};
use std::time::Duration;
use tracing::{debug, error};

pub struct GenAIClient {
    client: Client,
    model: String,
    provider: AdapterKind,
    timeout: Duration,
}

impl GenAIClient {
    pub async fn new(
        provider: AdapterKind,
        model: String,
        timeout: Duration,
    ) -> Result<Self, BackendError> {
        let custom_endpoint = std::env::var("AIPACK_API_BASE_URL").ok();

        let client = if let Some(endpoint_url) = custom_endpoint {
            debug!(
                "Using custom endpoint for {}: {}",
                provider.as_str(),
                endpoint_url
            );

            let model_for_resolver = model.clone();
            let resolver = ServiceTargetResolver::from_resolver_fn(
                move |_service_target: ServiceTarget| -> Result<ServiceTarget, genai::resolver::Error>
                {
                    let endpoint = Endpoint::from_owned(endpoint_url.clone());

                    let auth = match provider.default_key_env_name() {
                        Some(api_key_var) => AuthData::from_env(api_key_var),
                        None => AuthData::from_single(""),
                    };

                    let model_iden = ModelIden::new(provider, &model_for_resolver);

                    Ok(ServiceTarget {
                        endpoint,
                        auth,
                        model: model_iden,
                    })
                },
            );

            Client::builder()
                .with_service_target_resolver(resolver)
                .build()
        } else {
            Client::default()
        };

        debug!(
            "Creating GenAI client: provider={}, model={}",
            provider.as_str(),
            model,
        );

        Ok(Self {
            client,
            model,
            provider,
            timeout,
        })
    }

    fn convert_message(&self, msg: &ChatMessage) -> GenAIChatMessage {
        match msg.role {
            MessageRole::System => GenAIChatMessage::system(&msg.content),
            MessageRole::User => GenAIChatMessage::user(&msg.content),
            MessageRole::Assistant => {
                if let Some(ref tool_calls) = msg.tool_calls {
                    let genai_calls: Vec<genai::chat::ToolCall> = tool_calls
                        .iter()
                        .map(|tc| genai::chat::ToolCall {
                            call_id: tc.call_id.clone(),
                            fn_name: tc.name.clone(),
                            fn_arguments: tc.arguments.clone(),
                        })
                        .collect();
                    let content = MessageContent::from_tool_calls(genai_calls);
                    GenAIChatMessage::assistant(content)
                } else {
                    GenAIChatMessage::assistant(&msg.content)
                }
            }
            MessageRole::Tool => ToolResponse {
                call_id: msg.tool_call_id.clone().unwrap_or_default(),
                content: msg.content.clone(),
            }
            .into(),
        }
    }

    fn convert_tool(&self, tool: &ToolDefinition) -> GenAITool {
        GenAITool::new(&tool.name)
            .with_description(&tool.description)
            .with_schema(tool.parameters.clone())
    }
}

#[async_trait]
impl LLMClient for GenAIClient {
    async fn chat(&self, request: LLMRequest) -> Result<LLMResponse, BackendError> {
        let start = std::time::Instant::now();

        let messages: Vec<GenAIChatMessage> = request
            .messages
            .iter()
            .map(|m| self.convert_message(m))
            .collect();

        let tools: Vec<GenAITool> = request.tools.iter().map(|t| self.convert_tool(t)).collect();

        let genai_request = GenAIChatRequest::new(messages).with_tools(tools);

        let mut options = ChatOptions::default();
        if let Some(temp) = request.temperature {
            options = options.with_temperature(temp as f64);
        }
        if let Some(max_tokens) = request.max_tokens {
            options = options.with_max_tokens(max_tokens);
        }
        if let Some(ref sequences) = request.stop_sequences {
            options = options.with_stop_sequences(sequences.clone());
        }

        let response = match tokio::time::timeout(
            self.timeout,
            self.client
                .exec_chat(&self.model, genai_request, Some(&options)),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                error!("{} API error: {}", self.provider.as_str(), e);
                return Err(BackendError::ApiError {
                    message: format!("{} request failed: {}", self.provider.as_str(), e),
                    status_code: None,
                });
            }
            Err(_) => {
                error!(
                    "{} request timed out after {}s",
                    self.provider.as_str(),
                    self.timeout.as_secs()
                );
                return Err(BackendError::TimeoutError {
                    seconds: self.timeout.as_secs(),
                });
            }
        };

        let content = response.first_text().unwrap_or_default().to_string();

        // Take first tool call only
        let tool_call = response
            .tool_calls()
            .into_iter()
            .next()
            .map(|tc| ToolCall {
                call_id: tc.call_id.clone(),
                name: tc.fn_name.clone(),
                arguments: tc.fn_arguments.clone(),
            });

        if let Some(tc) = tool_call {
            Ok(LLMResponse::with_tool_call(
                content,
                tc,
                start.elapsed(),
            ))
        } else {
            Ok(LLMResponse::text(content, start.elapsed()))
        }
    }

    fn name(&self) -> &str {
        self.provider.as_str()
    }

    fn model_info(&self) -> Option<String> {
        Some(self.model.clone())
    }
}

impl std::fmt::Debug for GenAIClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenAIClient")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_genai_client_creation() {
        let client = GenAIClient::new(
            AdapterKind::Ollama,
            "qwen2.5-coder:7b".to_string(),
            Duration::from_secs(30),
        )
        .await
        .unwrap();

        assert_eq!(client.name(), "Ollama");
        assert_eq!(client.model_info(), Some("qwen2.5-coder:7b".to_string()));
    }

    #[test]
    fn test_debug_impl() {
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<GenAIClient>();
    }
}
