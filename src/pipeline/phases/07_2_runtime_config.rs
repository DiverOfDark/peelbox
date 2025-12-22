use crate::llm::{ChatMessage, LLMClient, LLMRequest};
use crate::pipeline::phase_trait::ServicePhase;
use crate::pipeline::service_context::ServiceContext;
use crate::stack::runtime::{HealthCheck, RuntimeConfig};
use anyhow::{Context as AnyhowContext, Result};
use async_trait::async_trait;
use serde::Deserialize;

pub struct RuntimeConfigPhase;

#[derive(Debug, Deserialize)]
struct LLMRuntimeResponse {
    entrypoint: Option<String>,
    port: Option<u16>,
    env_vars: Option<Vec<String>>,
    health_endpoint: Option<String>,
    native_deps: Option<Vec<String>>,
}

fn build_llm_prompt(context: &ServiceContext) -> Result<String> {
    let stack = context
        .stack
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Stack must be detected"))?;

    let service = &context.service;
    let stack_registry = context.stack_registry();

    let runtime_name = stack.runtime.name();
    let language_name = stack.language.name();
    let build_system_name = stack.build_system.name();

    let framework_info = stack
        .framework
        .as_ref()
        .map(|fw_id| {
            let fw = stack_registry.get_framework(fw_id.clone()).expect("Framework must exist");
            format!(
                "\nFramework: {}\nDefault port: {:?}\nHealth endpoints: {:?}",
                fw_id.name(),
                fw.default_ports().first(),
                fw.health_endpoints()
            )
        })
        .unwrap_or_default();

    Ok(format!(
        r#"Extract runtime configuration for this service.

Service path: {}
Language: {}
Build system: {}
Runtime: {}{}

Analyze the service files and extract:
- entrypoint: Start command (e.g., "java -jar app.jar", "node index.js", "python app.py")
- port: Exposed port number (check source files, configs)
- env_vars: Environment variable names used (e.g., ["DATABASE_URL", "API_KEY"])
- health_endpoint: Health check path (e.g., "/health", "/actuator/health")
- native_deps: System packages needed (e.g., ["build-base", "libpq-dev"])

Respond with JSON:
{{
  "entrypoint": "command to start the app" | null,
  "port": 8080 | null,
  "env_vars": ["VAR1", "VAR2"] | null,
  "health_endpoint": "/health" | null,
  "native_deps": ["package1"] | null
}}

Rules:
- Use framework defaults as hints (port, health endpoint)
- Scan source files for runtime-specific patterns
- Return null for fields you cannot determine
- Be concise, extract only what's explicitly used
"#,
        service.path.display(),
        language_name,
        build_system_name,
        runtime_name,
        framework_info
    ))
}

async fn extract_with_llm(
    context: &ServiceContext,
    llm_client: &dyn LLMClient,
) -> Result<RuntimeConfig> {
    let prompt = build_llm_prompt(context)?;

    let request = LLMRequest::new(vec![
        ChatMessage::system("You are a runtime configuration expert. Extract runtime config from service descriptions."),
        ChatMessage::user(prompt),
    ]);

    let response = llm_client.chat(request).await?;

    let parsed: LLMRuntimeResponse = serde_json::from_str(&response.content)
        .context("Failed to parse LLM runtime config response")?;

    let health = parsed.health_endpoint.map(|endpoint| HealthCheck { endpoint });

    Ok(RuntimeConfig {
        entrypoint: parsed.entrypoint,
        port: parsed.port,
        env_vars: parsed.env_vars.unwrap_or_default(),
        health,
        native_deps: parsed.native_deps.unwrap_or_default(),
    })
}

#[async_trait]
impl ServicePhase for RuntimeConfigPhase {
    fn name(&self) -> &'static str {
        "RuntimeConfigPhase"
    }

    fn try_deterministic(&self, context: &mut ServiceContext) -> Result<Option<()>> {
        let stack = context
            .stack
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Stack must be detected before RuntimeConfigPhase"))?;

        let stack_registry = context.stack_registry();
        let runtime = stack_registry.get_runtime(stack.runtime.clone());

        let scan = context.scan()?;
        let files = &scan.file_tree;

        let framework = stack.framework.as_ref().and_then(|fw_id| {
            stack_registry.get_framework(fw_id.clone())
        });

        if let Some(config) = runtime.try_extract(files, framework) {
            context.runtime_config = Some(config);
            return Ok(Some(()));
        }

        Ok(None)
    }

    async fn execute_llm(&self, context: &mut ServiceContext) -> Result<()> {
        let llm_client = context.llm_client();

        let config = extract_with_llm(context, llm_client).await?;

        context.runtime_config = Some(config);

        Ok(())
    }
}
