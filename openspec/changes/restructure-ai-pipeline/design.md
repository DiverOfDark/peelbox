# Design: Restructured AI Analysis Pipeline

## Context

The current AI analysis pipeline is a monolithic implementation where:
- `GenAIBackend.detect()` is ~350 lines handling conversation, tool execution, validation, caching
- Tool logic is scattered across 4 files (definitions, registry, executor, backend)
- No separation between LLM communication and business logic
- Tight coupling makes testing, extension, and debugging difficult

This redesign introduces a layered architecture with clear responsibilities and dependency injection.

## Goals

- **Separation of concerns**: Each component has a single responsibility
- **Testability**: Each layer can be unit tested in isolation via injected dependencies
- **Extensibility**: Easy to add new tools, providers, or analysis strategies
- **Observability**: Progress callbacks, structured logging, metrics hooks
- **Simplicity**: Fewer lines of code through better abstractions

## Non-Goals

- Backwards compatibility with internal APIs (external CLI interface unchanged)
- Supporting multiple concurrent analyses (single-threaded is fine)
- Persistent caching across runs (session-only cache is sufficient for now)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      DetectionService                            │
│  (Public API - unchanged, owns PipelineFactory)                  │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PipelineFactory                             │
│  Creates configured pipeline with injected dependencies          │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      AnalysisPipeline                            │
│  Orchestrates the detection workflow                             │
│  - References injected dependencies                              │
│  - Manages analysis lifecycle                                    │
│  - Emits progress events                                         │
└─────────────────────────────────────────────────────────────────┘
                                │
          ┌─────────────────────┼─────────────────────┐
          ▼                     ▼                     ▼
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│  Conversation   │  │   &ToolSystem   │  │   &Validator    │
│    Manager      │  │   (borrowed)    │  │   (borrowed)    │
│                 │  │                 │  │                 │
│  - Message hist │  │  - Registry     │  │  - Schema       │
│  - LLM calls    │  │  - Executor     │  │  - Business     │
│  - Stop logic   │  │  - Cache        │  │    rules        │
└─────────────────┘  └─────────────────┘  └─────────────────┘
          │                     │
          ▼                     ▼
┌─────────────────┐  ┌─────────────────┐
│  &dyn LLMClient │  │ &dyn FileSystem │
│   (borrowed)    │  │   (borrowed)    │
└─────────────────┘  └─────────────────┘
```

## Dependency Injection Pattern

Dependencies are created by a factory and passed by reference where possible:

```rust
/// Owns all long-lived dependencies
pub struct PipelineContext {
    pub llm_client: Box<dyn LLMClient>,
    pub file_system: Box<dyn FileSystem>,
    pub validator: Validator,
    pub config: PipelineConfig,
}

/// Creates pipeline with borrowed dependencies
pub struct PipelineFactory;

impl PipelineFactory {
    pub fn create<'a>(
        ctx: &'a PipelineContext,
        repo_path: PathBuf,
    ) -> Result<AnalysisPipeline<'a>, FactoryError> {
        let tool_system = ToolSystem::new(repo_path, &*ctx.file_system)?;

        Ok(AnalysisPipeline {
            conversation: ConversationManager::new(&*ctx.llm_client),
            tools: tool_system,
            validator: &ctx.validator,
            config: &ctx.config,
        })
    }
}
```

### Why References Over Ownership

1. **LLMClient** - Expensive to create (validates model, establishes connection). Reuse across analyses.
2. **FileSystem** - Stateless, no reason to recreate.
3. **Validator** - Stateless, rules don't change between analyses.
4. **ToolSystem** - Created per-analysis (holds repo_path and cache), but borrows FileSystem.

## Component Details

### 1. PipelineContext

Owns all long-lived dependencies. Created once at startup.

```rust
pub struct PipelineContext {
    llm_client: Box<dyn LLMClient>,
    file_system: Box<dyn FileSystem>,
    validator: Validator,
    config: PipelineConfig,
}

impl PipelineContext {
    pub fn new(config: &AipackConfig) -> Result<Self, ConfigError>;

    /// For testing - inject mock dependencies
    pub fn with_mocks(
        llm: Box<dyn LLMClient>,
        fs: Box<dyn FileSystem>,
    ) -> Self;
}
```

### 2. AnalysisPipeline

The main orchestrator. Borrows dependencies from context.

```rust
pub struct AnalysisPipeline<'ctx> {
    conversation: ConversationManager<'ctx>,
    tools: ToolSystem<'ctx>,
    validator: &'ctx Validator,
    config: &'ctx PipelineConfig,
}

impl<'ctx> AnalysisPipeline<'ctx> {
    pub async fn analyze(
        &mut self,
        jumpstart: Option<JumpstartContext>,
        progress: Option<&dyn ProgressHandler>,
    ) -> Result<UniversalBuild, AnalysisError>;
}
```

**Responsibilities:**
- Coordinates the analysis workflow
- Manages the iteration loop (max iterations, timeouts)
- Emits progress events (tool calls, validation results)
- Handles terminal conditions (success, failure, timeout)

### 3. ConversationManager

Manages LLM communication and message history. Borrows LLMClient.

```rust
pub struct ConversationManager<'a> {
    client: &'a dyn LLMClient,
    messages: Vec<Message>,
    options: ChatOptions,
}

impl<'a> ConversationManager<'a> {
    pub fn new(client: &'a dyn LLMClient) -> Self;
    pub fn set_system_prompt(&mut self, prompt: &str);
    pub fn add_user_message(&mut self, content: &str);
    pub fn add_assistant_message(&mut self, content: MessageContent);
    pub fn add_tool_response(&mut self, call_id: &str, content: &str);

    pub async fn get_response(
        &mut self,
        tools: &[ToolDefinition],
    ) -> Result<LLMResponse, ConversationError>;

    pub fn message_count(&self) -> usize;
    pub fn clear(&mut self);
}
```

### 4. LLMClient Trait

Minimal trait for LLM communication.

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> Result<LLMResponse, LLMError>;

    fn name(&self) -> &str;
    fn model(&self) -> &str;
}
```

**Implementations:**

```rust
/// Production implementation using genai crate
pub struct GenAIClient {
    client: genai::Client,
    model: String,
    provider: Provider,
}

/// Test implementation with scripted responses
pub struct MockLLMClient {
    responses: VecDeque<LLMResponse>,
    recorded_calls: Vec<RecordedCall>,
}
```

#### 4.1 Context Window Requirements

**Target context window: 16K tokens**

This constrains our design to be token-efficient. All components must respect this budget.

##### Token Budget Breakdown

| Component | Budget | Notes |
|-----------|--------|-------|
| System prompt | 1,500 | Instructions + tool schemas |
| Bootstrap context | 1,000 | Repo summary, key manifests (truncated) |
| Conversation history | 10,000 | Tool calls + results + reasoning |
| Output buffer | 1,500 | Final UniversalBuild JSON |
| Safety margin | 2,000 | Prevent truncation |
| **Total** | **16,000** | |

##### Context Management Strategies

```rust
pub struct ContextConfig {
    /// Maximum context window size in tokens
    pub max_context_tokens: usize,  // Default: 16000

    /// Maximum tokens for a single tool result
    pub max_tool_result_tokens: usize,  // Default: 2000

    /// Maximum lines to read from a file
    pub max_file_lines: usize,  // Default: 150

    /// Maximum manifests to include in bootstrap
    pub max_bootstrap_manifests: usize,  // Default: 3

    /// Maximum characters per manifest in bootstrap
    pub max_manifest_chars: usize,  // Default: 3000

    /// When to start summarizing old messages
    pub summarize_after_turns: usize,  // Default: 6
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 16000,
            max_tool_result_tokens: 2000,
            max_file_lines: 150,
            max_bootstrap_manifests: 3,
            max_manifest_chars: 3000,
            summarize_after_turns: 6,
        }
    }
}
```

##### Truncation Rules

1. **File reads**: Truncate at `max_file_lines` with note "... truncated, use line_start/line_end for more"
2. **Grep results**: Limit to 50 matches, show "... and N more matches"
3. **Directory listings**: Limit to 100 entries, show "... and N more files"
4. **Tree output**: Limit depth to 3, show "... deeper directories omitted"
5. **Manifest content**: Truncate at `max_manifest_chars` in bootstrap

##### History Summarization

After `summarize_after_turns`, older tool results are summarized:

```rust
impl ConversationManager {
    fn maybe_summarize_history(&mut self) {
        if self.messages.len() > self.config.summarize_after_turns * 2 {
            // Keep: system prompt, last N turns, current turn
            // Summarize: older tool results -> "Previously read: Cargo.toml (Rust project), package.json (Node.js)"
            self.summarize_old_turns();
        }
    }
}
```

#### 4.2 Model Requirements

##### Required Capabilities

| Capability | Required | Notes |
|------------|----------|-------|
| Function/Tool calling | ✅ Yes | Core to our architecture |
| JSON mode | ✅ Yes | For structured output |
| Context window | ≥16K | Minimum viable |
| Code understanding | ✅ Yes | Build system detection |

##### Recommended Local Models (Ollama)

**Tier 1 - Best Quality (if hardware allows):**

| Model | Command | Context | VRAM | Notes |
|-------|---------|---------|------|-------|
| Qwen2.5-Coder 14B | `ollama pull qwen2.5-coder:14b` | 32K | 10GB | Best balance |
| Qwen2.5-Coder 32B | `ollama pull qwen2.5-coder:32b` | 32K | 20GB | Highest quality |

**Tier 2 - Good Quality (mainstream hardware):**

| Model | Command | Context | VRAM | Notes |
|-------|---------|---------|------|-------|
| Qwen2.5-Coder 7B | `ollama pull qwen2.5-coder:7b` | 32K | 6GB | **Recommended default** |
| Llama 3.1 8B | `ollama pull llama3.1:8b` | 128K | 6GB | Good alternative |
| Mistral Nemo 12B | `ollama pull mistral-nemo:12b` | 128K | 8GB | Good function calling |

**Tier 3 - CPU-Only (no GPU):**

| Model | Command | Context | RAM | Notes |
|-------|---------|---------|-----|-------|
| Qwen2.5-Coder 7B Q4 | `ollama pull qwen2.5-coder:7b-q4_0` | 32K | 8GB | Slower but works |

##### Cloud Provider Models

| Provider | Model | Context | Notes |
|----------|-------|---------|-------|
| Anthropic | claude-sonnet-4-20250514 | 200K | Excellent, fast |
| Anthropic | claude-opus-4-20250514 | 200K | Best quality |
| OpenAI | gpt-4o | 128K | Good function calling |
| OpenAI | gpt-4o-mini | 128K | Budget option |
| Google | gemini-1.5-flash | 1M | Fast, large context |

##### Model Validation

```rust
impl GenAIClient {
    pub async fn validate_model(&self) -> Result<ModelCapabilities, LLMError> {
        // Check if model supports required features
        let caps = ModelCapabilities {
            supports_tools: self.test_tool_calling().await?,
            supports_json_mode: self.test_json_mode().await?,
            context_window: self.get_context_window(),
            tokens_per_second: self.benchmark_speed().await?,
        };

        if !caps.supports_tools {
            return Err(LLMError::UnsupportedModel(
                "Model does not support function calling".into()
            ));
        }

        if caps.context_window < 16000 {
            warn!("Model context window ({}) below recommended 16K", caps.context_window);
        }

        Ok(caps)
    }
}
```

#### 4.3 Token Counting

Approximate token counting for budget management:

```rust
pub fn estimate_tokens(text: &str) -> usize {
    // Rough estimate: ~4 characters per token for English/code
    // More accurate would use tiktoken or model-specific tokenizer
    (text.len() + 3) / 4
}

pub fn estimate_message_tokens(msg: &Message) -> usize {
    let base = 4; // Message overhead
    let content = estimate_tokens(&msg.content);
    let role = 2; // Role token
    base + content + role
}

impl ConversationManager {
    pub fn current_token_count(&self) -> usize {
        self.messages.iter().map(estimate_message_tokens).sum()
    }

    pub fn remaining_tokens(&self) -> usize {
        self.config.max_context_tokens.saturating_sub(self.current_token_count())
    }

    pub fn can_add_message(&self, estimated_tokens: usize) -> bool {
        self.remaining_tokens() > estimated_tokens + self.config.output_buffer
    }
}
```

#### 4.4 Embedded LLM (Zero-Config Local Inference)

For a zero-config experience, aipack includes embedded LLM inference using [Candle](https://github.com/huggingface/candle).

##### Why Candle

| Requirement | Candle |
|-------------|--------|
| Single static binary | ✅ ~22MB |
| Pure Rust (no C++ toolchain) | ✅ |
| CUDA support | ✅ via feature |
| Metal support | ✅ via feature |
| HuggingFace model downloads | ✅ native |

##### Provider Selection Logic

```rust
pub async fn select_llm_client(config: &AipackConfig) -> Result<Box<dyn LLMClient>> {
    // 1. Explicit provider in config/CLI → use it
    if let Some(provider) = &config.provider {
        return create_genai_client(provider, &config.model).await;
    }

    // 2. Check for API keys → use cloud provider
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        info!("Using Claude (ANTHROPIC_API_KEY found)");
        return create_genai_client(Provider::Claude, "claude-sonnet-4-20250514").await;
    }
    if std::env::var("OPENAI_API_KEY").is_ok() {
        info!("Using OpenAI (OPENAI_API_KEY found)");
        return create_genai_client(Provider::OpenAI, "gpt-4o-mini").await;
    }

    // 3. Check for Ollama running locally
    if is_ollama_available().await {
        info!("Using Ollama (detected at {})", ollama_host());
        return create_genai_client(Provider::Ollama, "qwen2.5-coder:7b").await;
    }

    // 4. Fall back to embedded inference
    info!("No external LLM found, using embedded inference");
    create_embedded_client().await
}

async fn is_ollama_available() -> bool {
    let host = std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    reqwest::Client::new()
        .get(format!("{}/api/tags", host))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}
```

##### Model Selection by Available Memory

```rust
pub struct EmbeddedModelConfig {
    pub model_id: &'static str,
    pub file_name: &'static str,
    pub size_bytes: u64,
    pub min_ram_gb: u64,
}

const EMBEDDED_MODELS: &[EmbeddedModelConfig] = &[
    EmbeddedModelConfig {
        model_id: "Qwen/Qwen2.5-Coder-7B-Instruct",
        file_name: "qwen2.5-coder-7b",
        size_bytes: 4_700_000_000,  // ~4.4GB
        min_ram_gb: 8,
    },
    EmbeddedModelConfig {
        model_id: "Qwen/Qwen2.5-Coder-3B-Instruct",
        file_name: "qwen2.5-coder-3b",
        size_bytes: 2_000_000_000,  // ~2GB
        min_ram_gb: 4,
    },
    EmbeddedModelConfig {
        model_id: "Qwen/Qwen2.5-Coder-1.5B-Instruct",
        file_name: "qwen2.5-coder-1.5b",
        size_bytes: 1_100_000_000,  // ~1.1GB
        min_ram_gb: 2,
    },
];

fn select_model_for_system() -> &'static EmbeddedModelConfig {
    let available_ram_gb = get_available_ram_gb();

    // Select largest model that fits in available RAM (with 2GB headroom)
    EMBEDDED_MODELS
        .iter()
        .find(|m| available_ram_gb >= m.min_ram_gb + 2)
        .unwrap_or(&EMBEDDED_MODELS[2])  // Fallback to smallest
}

fn get_available_ram_gb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        use sysinfo::System;
        let sys = System::new_all();
        sys.available_memory() / 1_073_741_824  // bytes to GB
    }
    #[cfg(target_os = "macos")]
    {
        use sysinfo::System;
        let sys = System::new_all();
        sys.available_memory() / 1_073_741_824
    }
    #[cfg(target_os = "windows")]
    {
        use sysinfo::System;
        let sys = System::new_all();
        sys.available_memory() / 1_073_741_824
    }
}
```

##### First-Run Download UX

```rust
impl EmbeddedClient {
    pub async fn new() -> Result<Self, LLMError> {
        let model_config = select_model_for_system();
        let model_path = get_model_cache_path(model_config);

        if !model_path.exists() {
            // Check if interactive terminal
            if atty::is(atty::Stream::Stdout) {
                prompt_for_download(model_config)?;
            } else {
                // CI/non-interactive: fail with instructions
                return Err(LLMError::ModelNotFound {
                    message: format!(
                        "Model not found. Run `aipack setup` to download, or set ANTHROPIC_API_KEY/OPENAI_API_KEY"
                    ),
                });
            }

            download_model(model_config, &model_path).await?;
        }

        Self::load_model(&model_path).await
    }
}

fn prompt_for_download(model: &EmbeddedModelConfig) -> Result<(), LLMError> {
    use dialoguer::Confirm;

    let size_mb = model.size_bytes / 1_000_000;
    let confirmed = Confirm::new()
        .with_prompt(format!(
            "Download {} ({} MB) for local inference?",
            model.model_id, size_mb
        ))
        .default(true)
        .interact()
        .map_err(|e| LLMError::Other { message: e.to_string() })?;

    if !confirmed {
        return Err(LLMError::Other {
            message: "Model download declined. Set ANTHROPIC_API_KEY or OPENAI_API_KEY to use cloud inference.".into()
        });
    }

    Ok(())
}

async fn download_model(model: &EmbeddedModelConfig, path: &Path) -> Result<(), LLMError> {
    use hf_hub::api::sync::Api;
    use indicatif::{ProgressBar, ProgressStyle};

    let api = Api::new()?;
    let repo = api.model(model.model_id.to_string());

    let pb = ProgressBar::new(model.size_bytes);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .progress_chars("#>-"));

    // Download with progress
    let file = repo.get_with_progress("model.safetensors", |downloaded, _total| {
        pb.set_position(downloaded);
    })?;

    pb.finish_with_message("Download complete");

    // Move to cache location
    std::fs::rename(file, path)?;

    Ok(())
}

fn get_model_cache_path(model: &EmbeddedModelConfig) -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("aipack")
        .join("models")
        .join(model.file_name)
}
```

##### Hardware Acceleration Detection

```rust
#[derive(Debug, Clone, Copy)]
pub enum ComputeBackend {
    Cpu,
    Cuda,
    Metal,
}

fn detect_best_backend() -> ComputeBackend {
    #[cfg(feature = "cuda")]
    {
        if cuda_is_available() {
            info!("CUDA detected, using GPU acceleration");
            return ComputeBackend::Cuda;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if metal_is_available() {
            info!("Metal detected, using GPU acceleration");
            return ComputeBackend::Metal;
        }
    }

    info!("Using CPU backend");
    ComputeBackend::Cpu
}

#[cfg(feature = "cuda")]
fn cuda_is_available() -> bool {
    candle_core::cuda::is_available()
}

#[cfg(target_os = "macos")]
fn metal_is_available() -> bool {
    candle_core::metal::is_available()
}
```

##### EmbeddedClient Implementation

```rust
pub struct EmbeddedClient {
    model: candle_transformers::models::qwen2::Model,
    tokenizer: tokenizers::Tokenizer,
    device: candle_core::Device,
}

#[async_trait]
impl LLMClient for EmbeddedClient {
    async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        options: &ChatOptions,
    ) -> Result<LLMResponse, LLMError> {
        // Format messages for Qwen chat template
        let prompt = self.format_chat_prompt(messages, tools)?;

        // Tokenize
        let tokens = self.tokenizer.encode(prompt, true)
            .map_err(|e| LLMError::Other { message: e.to_string() })?;

        // Generate
        let output_tokens = self.generate(
            &tokens,
            options.max_tokens.unwrap_or(4096),
            options.temperature.unwrap_or(0.1),
        )?;

        // Decode and parse
        let output_text = self.tokenizer.decode(&output_tokens, true)
            .map_err(|e| LLMError::Other { message: e.to_string() })?;

        self.parse_response(&output_text)
    }

    fn name(&self) -> &str { "embedded" }
    fn model(&self) -> &str { "qwen2.5-coder" }
}
```

##### Feature Flags

```toml
# Cargo.toml
[features]
default = ["embedded-llm"]
embedded-llm = ["candle-core", "candle-transformers", "candle-nn", "hf-hub", "tokenizers"]
cuda = ["candle-core/cuda", "embedded-llm"]
metal = ["candle-core/metal", "embedded-llm"]
```

### 5. ToolSystem

Unified tool management. Created per-analysis, borrows FileSystem.

```rust
pub struct ToolSystem<'a> {
    executor: ToolExecutor<'a>,
    registry: ToolRegistry,
    cache: ToolCache,
}

impl<'a> ToolSystem<'a> {
    pub fn new(repo_path: PathBuf, fs: &'a dyn FileSystem) -> Result<Self, ToolError>;

    pub fn definitions(&self) -> &[ToolDefinition];

    pub async fn execute(&mut self, call: &ToolCall) -> ToolResult;

    pub fn is_terminal(&self, tool_name: &str) -> bool;

    pub fn cache_stats(&self) -> CacheStats;
}
```

### 6. Language Registry

All language-specific knowledge is consolidated into per-language definition files. Each language defines its extensions, manifests, detection rules, and best practice templates in one place.

#### 6.1 LanguageDefinition Trait

```rust
/// All language-specific knowledge in one place
pub trait LanguageDefinition: Send + Sync {
    /// Unique identifier (e.g., "rust", "java", "python")
    fn id(&self) -> &'static str;

    /// Human-readable name (e.g., "Rust", "Java", "Python")
    fn name(&self) -> &'static str;

    /// File extensions that indicate this language (without dot)
    fn extensions(&self) -> &'static [&'static str];

    /// Manifest files that indicate this language's build system
    fn manifests(&self) -> &[ManifestDefinition];

    /// Try to detect build system from manifest contents
    /// Returns None if this language doesn't match
    fn detect_build_system(&self, manifests: &[ManifestWithContent]) -> Option<BuildSuggestion>;

    /// Get best practices template for a specific build system variant
    fn best_practices(&self, build_system: &str, variant: Option<&str>) -> Option<&str>;
}

/// Manifest file definition
pub struct ManifestDefinition {
    /// Filename to match (exact match)
    pub filename: &'static str,
    /// Priority (1 = primary, 2 = secondary, 3 = lock file, 4 = container)
    pub priority: u8,
    /// Is this a lock file?
    pub is_lock_file: bool,
}
```

#### 6.2 Example: Rust Language Definition

```rust
// src/languages/rust.rs

pub struct RustLanguage;

impl LanguageDefinition for RustLanguage {
    fn id(&self) -> &'static str { "rust" }
    fn name(&self) -> &'static str { "Rust" }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn manifests(&self) -> &[ManifestDefinition] {
        &[
            ManifestDefinition { filename: "Cargo.toml", priority: 1, is_lock_file: false },
            ManifestDefinition { filename: "Cargo.lock", priority: 3, is_lock_file: true },
        ]
    }

    fn detect_build_system(&self, manifests: &[ManifestWithContent]) -> Option<BuildSuggestion> {
        // Find Cargo.toml at root
        let cargo_toml = manifests.iter()
            .find(|m| m.filename == "Cargo.toml" && m.depth == 0)?;

        let is_workspace = cargo_toml.content.contains("[workspace]");

        Some(BuildSuggestion {
            language: "Rust".into(),
            build_system: "Cargo".into(),
            confidence: 0.95,
            reasoning: if is_workspace {
                "Cargo.toml at root with [workspace] section".into()
            } else {
                "Cargo.toml at root".into()
            },
            variant: if is_workspace { Some("workspace".into()) } else { None },
        })
    }

    fn best_practices(&self, build_system: &str, variant: Option<&str>) -> Option<&str> {
        match (build_system, variant) {
            ("Cargo", Some("workspace")) => Some(RUST_WORKSPACE_TEMPLATE),
            ("Cargo", _) => Some(RUST_TEMPLATE),
            _ => None,
        }
    }
}

const RUST_TEMPLATE: &str = r#"
# Rust Build Best Practices

## Build Stage
- Base image: rust:1.75-slim or rust:1.75-alpine
- Use cargo build --release for production builds
- Cache cargo registry and target directory
- Consider cargo-chef for efficient Docker layer caching

## Runtime Stage
- Use distroless or alpine for minimal image size
- Copy only the compiled binary
- Set appropriate user permissions (non-root)

## Commands
- Build: cargo build --release
- Test: cargo test
- Lint: cargo clippy -- -D warnings
- Format check: cargo fmt -- --check
"#;

const RUST_WORKSPACE_TEMPLATE: &str = r#"
# Rust Workspace Best Practices

## Build Stage
- Base image: rust:1.75-slim
- Build specific package: cargo build --release -p <package>
- Cache cargo registry at workspace root
- Consider building all workspace members if needed

## Runtime Stage
- Copy specific binary from target/release/
- Multi-stage build recommended for each deployable
"#;
```

#### 6.3 Example: Java Language Definition

```rust
// src/languages/java.rs

pub struct JavaLanguage;

impl LanguageDefinition for JavaLanguage {
    fn id(&self) -> &'static str { "java" }
    fn name(&self) -> &'static str { "Java" }

    fn extensions(&self) -> &'static [&'static str] {
        &["java"]
    }

    fn manifests(&self) -> &[ManifestDefinition] {
        &[
            ManifestDefinition { filename: "pom.xml", priority: 1, is_lock_file: false },
            ManifestDefinition { filename: "build.gradle", priority: 1, is_lock_file: false },
            ManifestDefinition { filename: "build.gradle.kts", priority: 1, is_lock_file: false },
            ManifestDefinition { filename: "settings.gradle", priority: 2, is_lock_file: false },
            ManifestDefinition { filename: "settings.gradle.kts", priority: 2, is_lock_file: false },
            ManifestDefinition { filename: "gradle.properties", priority: 2, is_lock_file: false },
            ManifestDefinition { filename: "mvnw", priority: 2, is_lock_file: false },
            ManifestDefinition { filename: "gradlew", priority: 2, is_lock_file: false },
        ]
    }

    fn detect_build_system(&self, manifests: &[ManifestWithContent]) -> Option<BuildSuggestion> {
        let root_manifests: Vec<_> = manifests.iter().filter(|m| m.depth == 0).collect();

        // Check for Maven
        if root_manifests.iter().any(|m| m.filename == "pom.xml") {
            return Some(BuildSuggestion {
                language: "Java".into(),
                build_system: "Maven".into(),
                confidence: 0.90,
                reasoning: "pom.xml at root".into(),
                variant: None,
            });
        }

        // Check for Gradle
        if let Some(gradle) = root_manifests.iter()
            .find(|m| m.filename == "build.gradle" || m.filename == "build.gradle.kts")
        {
            let is_kotlin_dsl = gradle.filename.ends_with(".kts");
            return Some(BuildSuggestion {
                language: "Java".into(),
                build_system: "Gradle".into(),
                confidence: 0.90,
                reasoning: format!("{} at root", gradle.filename),
                variant: if is_kotlin_dsl { Some("kotlin-dsl".into()) } else { None },
            });
        }

        None
    }

    fn best_practices(&self, build_system: &str, variant: Option<&str>) -> Option<&str> {
        match build_system {
            "Maven" => Some(JAVA_MAVEN_TEMPLATE),
            "Gradle" => Some(JAVA_GRADLE_TEMPLATE),
            _ => None,
        }
    }
}

const JAVA_MAVEN_TEMPLATE: &str = r#"
# Java Maven Best Practices

## Build Stage
- Base image: eclipse-temurin:21-jdk or maven:3.9-eclipse-temurin-21
- Use Maven wrapper (./mvnw) if present
- Cache .m2/repository for faster builds
- Build: ./mvnw package -DskipTests (for Docker), ./mvnw verify (for CI)

## Runtime Stage
- Base image: eclipse-temurin:21-jre-alpine
- Copy JAR from target/*.jar
- Use ENTRYPOINT ["java", "-jar", "app.jar"]
"#;

const JAVA_GRADLE_TEMPLATE: &str = r#"
# Java Gradle Best Practices

## Build Stage
- Base image: eclipse-temurin:21-jdk or gradle:8-jdk21
- Use Gradle wrapper (./gradlew) if present
- Cache ~/.gradle for faster builds
- Build: ./gradlew build -x test (for Docker), ./gradlew build (for CI)

## Runtime Stage
- Base image: eclipse-temurin:21-jre-alpine
- Copy JAR from build/libs/*.jar
- Use ENTRYPOINT ["java", "-jar", "app.jar"]
"#;
```

#### 6.4 Language Registry

```rust
// src/languages/mod.rs

mod rust;
mod java;
mod kotlin;
mod javascript;
mod typescript;
mod python;
mod go;
mod dotnet;
mod ruby;
mod php;
mod cpp;
mod elixir;

use std::collections::HashMap;

pub struct LanguageRegistry {
    languages: Vec<Box<dyn LanguageDefinition>>,
    extension_map: HashMap<&'static str, usize>,  // extension -> language index
    manifest_map: HashMap<&'static str, Vec<usize>>,  // filename -> language indices
}

impl LanguageRegistry {
    pub fn new() -> Self {
        let languages: Vec<Box<dyn LanguageDefinition>> = vec![
            Box::new(rust::RustLanguage),
            Box::new(java::JavaLanguage),
            Box::new(kotlin::KotlinLanguage),
            Box::new(javascript::JavaScriptLanguage),
            Box::new(typescript::TypeScriptLanguage),
            Box::new(python::PythonLanguage),
            Box::new(go::GoLanguage),
            Box::new(dotnet::DotNetLanguage),
            Box::new(ruby::RubyLanguage),
            Box::new(php::PhpLanguage),
            Box::new(cpp::CppLanguage),
            Box::new(elixir::ElixirLanguage),
        ];

        // Build extension -> language index map
        let mut extension_map = HashMap::new();
        for (idx, lang) in languages.iter().enumerate() {
            for ext in lang.extensions() {
                extension_map.insert(*ext, idx);
            }
        }

        // Build manifest -> language indices map (multiple languages may share manifests)
        let mut manifest_map: HashMap<&'static str, Vec<usize>> = HashMap::new();
        for (idx, lang) in languages.iter().enumerate() {
            for manifest in lang.manifests() {
                manifest_map.entry(manifest.filename).or_default().push(idx);
            }
        }

        Self { languages, extension_map, manifest_map }
    }

    /// Get language by extension
    pub fn language_for_extension(&self, ext: &str) -> Option<&dyn LanguageDefinition> {
        self.extension_map.get(ext).map(|&idx| self.languages[idx].as_ref())
    }

    /// Get all known extensions
    pub fn all_extensions(&self) -> impl Iterator<Item = (&'static str, &str)> + '_ {
        self.languages.iter().flat_map(|lang| {
            lang.extensions().iter().map(move |ext| (*ext, lang.name()))
        })
    }

    /// Get all known manifest filenames with priorities
    pub fn all_manifests(&self) -> Vec<(&'static str, u8)> {
        let mut manifests = Vec::new();
        for lang in &self.languages {
            for m in lang.manifests() {
                manifests.push((m.filename, m.priority));
            }
        }
        // Deduplicate by filename, keeping lowest priority
        manifests.sort_by(|a, b| a.0.cmp(b.0).then(a.1.cmp(&b.1)));
        manifests.dedup_by(|a, b| a.0 == b.0);
        manifests
    }

    /// Detect build system from manifests
    pub fn detect_build_system(&self, manifests: &[ManifestWithContent]) -> Option<BuildSuggestion> {
        // Try each language's detection in order
        // First language to return a suggestion wins
        // Languages are ordered by priority in the registry
        for lang in &self.languages {
            if let Some(suggestion) = lang.detect_build_system(manifests) {
                return Some(suggestion);
            }
        }
        None
    }

    /// Get best practices for a language/build system combination
    pub fn best_practices(&self, language: &str, build_system: &str, variant: Option<&str>) -> Option<&str> {
        self.languages.iter()
            .find(|l| l.name().eq_ignore_ascii_case(language) || l.id() == language)
            .and_then(|l| l.best_practices(build_system, variant))
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

#### 6.5 Adding a New Language

To add support for a new language:

1. Create `src/languages/<language>.rs`
2. Implement `LanguageDefinition` trait
3. Add to the registry in `src/languages/mod.rs`

Example for adding Elixir:

```rust
// src/languages/elixir.rs

pub struct ElixirLanguage;

impl LanguageDefinition for ElixirLanguage {
    fn id(&self) -> &'static str { "elixir" }
    fn name(&self) -> &'static str { "Elixir" }

    fn extensions(&self) -> &'static [&'static str] {
        &["ex", "exs", "eex", "heex", "leex", "sface"]
    }

    fn manifests(&self) -> &[ManifestDefinition] {
        &[
            ManifestDefinition { filename: "mix.exs", priority: 1, is_lock_file: false },
            ManifestDefinition { filename: "mix.lock", priority: 3, is_lock_file: true },
            ManifestDefinition { filename: "rebar.config", priority: 1, is_lock_file: false },
        ]
    }

    fn detect_build_system(&self, manifests: &[ManifestWithContent]) -> Option<BuildSuggestion> {
        let mix_exs = manifests.iter()
            .find(|m| m.filename == "mix.exs" && m.depth == 0)?;

        let is_phoenix = mix_exs.content.contains(":phoenix");
        let is_umbrella = mix_exs.content.contains("apps_path:");

        Some(BuildSuggestion {
            language: "Elixir".into(),
            build_system: "Mix".into(),
            confidence: 0.95,
            reasoning: "mix.exs at root".into(),
            variant: match (is_phoenix, is_umbrella) {
                (true, true) => Some("phoenix-umbrella".into()),
                (true, false) => Some("phoenix".into()),
                (false, true) => Some("umbrella".into()),
                (false, false) => None,
            },
        })
    }

    fn best_practices(&self, build_system: &str, variant: Option<&str>) -> Option<&str> {
        match (build_system, variant) {
            ("Mix", Some("phoenix")) | ("Mix", Some("phoenix-umbrella")) => Some(ELIXIR_PHOENIX_TEMPLATE),
            ("Mix", _) => Some(ELIXIR_TEMPLATE),
            _ => None,
        }
    }
}

const ELIXIR_TEMPLATE: &str = r#"
# Elixir Mix Best Practices

## Build Stage
- Base image: elixir:1.15-alpine or hexpm/elixir:1.15.0-erlang-26.0-alpine-3.18.0
- Install hex and rebar: mix local.hex --force && mix local.rebar --force
- Cache deps/ and _build/ directories
- Build: MIX_ENV=prod mix release

## Runtime Stage
- Base image: alpine:3.18 (Elixir releases are self-contained)
- Copy _build/prod/rel/<app_name>/ to /app
- Use ENTRYPOINT ["/app/bin/<app_name>", "start"]
"#;

const ELIXIR_PHOENIX_TEMPLATE: &str = r#"
# Elixir Phoenix Best Practices

## Build Stage
- Base image: hexpm/elixir:1.15.0-erlang-26.0-alpine-3.18.0
- Install Node.js for asset compilation
- Run: mix deps.get --only prod
- Run: mix assets.deploy
- Run: MIX_ENV=prod mix release

## Runtime Stage
- Base image: alpine:3.18
- Copy release from _build/prod/rel/
- Expose port 4000
- Set DATABASE_URL, SECRET_KEY_BASE at runtime
"#;
```

### 7. Repository Bootstrap System

Before the first LLM query, we perform an intelligent scan to provide context. This reduces tool calls and helps the LLM make better decisions faster.

The bootstrap system uses `LanguageRegistry` for all language-specific knowledge (extensions, manifests, detection rules).

#### 7.1 Bootstrap Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    AnalysisPipeline.analyze()                    │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Phase 1: Bootstrap Scan                       │
│  (Synchronous, before any LLM calls)                            │
│                                                                  │
│  1. Quick directory walk (respect max_files, skip_dirs)         │
│  2. Identify manifest files by name pattern                      │
│  3. Detect languages by file extensions                          │
│  4. Identify key directories (src/, tests/, etc.)               │
│  5. Read manifest contents (up to max_manifest_size)            │
│  6. Generate build suggestion via heuristics                     │
│  7. Format context for system prompt                             │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Phase 2: System Prompt Assembly               │
│                                                                  │
│  BASE_SYSTEM_PROMPT + BOOTSTRAP_CONTEXT + TOOL_DESCRIPTIONS     │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Phase 3: LLM Conversation                     │
│                                                                  │
│  LLM already knows:                                              │
│  - Repository structure and size                                 │
│  - Primary language and build system (suggested)                 │
│  - Contents of key manifest files                                │
│  - What to look for if suggestion is wrong                       │
└─────────────────────────────────────────────────────────────────┘
```

#### 7.2 Bootstrap Scan Algorithm

```rust
impl BootstrapScanner {
    /// LanguageRegistry is injected - provides all language-specific knowledge
    pub fn new(registry: &LanguageRegistry, config: BootstrapConfig) -> Self {
        Self { registry, config }
    }

    pub fn scan(&self, repo_path: &Path) -> Result<BootstrapContext, BootstrapError> {
        // Step 1: Quick stats walk
        let stats = self.collect_stats(repo_path)?;

        // Step 2: Find manifest files using registry's manifest list
        let known_manifests = self.registry.all_manifests();
        let manifest_paths = self.find_manifests(repo_path, &stats.files, &known_manifests)?;

        // Step 3: Prioritize and read manifests
        let manifests = self.read_manifests(&manifest_paths)?;

        // Step 4: Detect languages from extensions using registry
        let language_detection = self.detect_languages(&stats)?;

        // Step 5: Identify key directories
        let key_dirs = self.identify_key_directories(&stats)?;

        // Step 6: Generate build suggestion using registry's detection
        let suggestion = self.registry.detect_build_system(&manifests);

        // Step 7: Assemble context
        Ok(BootstrapContext {
            summary: RepoSummary {
                file_count: stats.file_count,
                dir_count: stats.dir_count,
                total_size: stats.total_size,
                language_detection,
                root_files: stats.root_files,
                key_directories: key_dirs,
                workspace_indicators: self.detect_workspace_indicators(&manifests),
            },
            manifests,
            suggestion,
            scan_duration_ms: stats.duration_ms,
        })
    }
}
```

#### 7.3 Manifest Discovery & Prioritization

Manifests are discovered using `LanguageRegistry.all_manifests()` which aggregates manifests from all language definitions:

```rust
impl BootstrapScanner {
    fn find_manifests(
        &self,
        repo_path: &Path,
        files: &[FileInfo],
        known_manifests: &[(&str, u8)],  // (filename, priority) from registry
    ) -> Vec<ManifestPath> {
        let manifest_set: HashSet<&str> = known_manifests.iter().map(|(f, _)| *f).collect();

        let mut found: Vec<ManifestPath> = files
            .iter()
            .filter(|f| manifest_set.contains(f.name.as_str()))
            .map(|f| {
                let priority = known_manifests.iter()
                    .find(|(name, _)| *name == f.name)
                    .map(|(_, p)| *p)
                    .unwrap_or(99);
                ManifestPath {
                    path: f.path.clone(),
                    filename: f.name.clone(),
                    depth: f.depth,
                    priority,
                }
            })
            .collect();

        // Sort by: (depth ASC, priority ASC, path ASC)
        found.sort_by(|a, b| {
            a.depth.cmp(&b.depth)
                .then(a.priority.cmp(&b.priority))
                .then(a.path.cmp(&b.path))
        });

        found
    }
}
```

#### 7.4 Language Detection Heuristics

Language detection uses `LanguageRegistry.language_for_extension()` to map extensions to languages. Unknown extensions are tracked separately:

```rust
impl BootstrapScanner {
    fn detect_languages(&self, stats: &ScanStats) -> LanguageDetection {
        let mut lang_counts: HashMap<&str, (usize, HashSet<String>)> = HashMap::new();
        let mut unknown_exts: HashMap<String, usize> = HashMap::new();

        for (ext, count) in &stats.extension_counts {
            if let Some(lang) = self.registry.language_for_extension(ext) {
                let entry = lang_counts.entry(lang.name()).or_insert((0, HashSet::new()));
                entry.0 += count;
                entry.1.insert(ext.clone());
            } else if !ext.is_empty() {
                *unknown_exts.entry(ext.clone()).or_insert(0) += count;
            }
        }

        // ... rest of detection logic (see section 7.4 for full implementation)
    }
}
```

The full `LanguageDetection` struct tracks both recognized and unknown extensions:

```rust
/// Result of language detection including unknown extensions
pub struct LanguageDetection {
    /// Recognized languages with stats
    pub languages: Vec<LanguageStats>,
    /// Unknown extensions - top N by file count
    pub unknown_top: Vec<ExtensionStats>,
    /// Unknown extensions - long tail (others)
    pub unknown_tail: Vec<ExtensionStats>,
    /// Total files with recognized extensions
    pub recognized_count: usize,
    /// Total files with unknown extensions
    pub unknown_count: usize,
}

pub struct LanguageStats {
    pub language: String,
    pub file_count: usize,
    pub percentage: f32,
    pub extensions: Vec<String>,
}

pub struct ExtensionStats {
    pub extension: String,
    pub file_count: usize,
}
```

#### 7.5 Build Suggestion (via Registry)

Build suggestions are now delegated to the `LanguageRegistry`:

```rust
impl BootstrapScanner {
    fn suggest_build_system(&self, manifests: &[ManifestWithContent]) -> Option<BuildSuggestion> {
        // Delegate to registry - it iterates through all language definitions
        // and returns the first match
        self.registry.detect_build_system(manifests)
    }
}
```

Each language's `detect_build_system()` method (see Section 6) contains the language-specific detection logic. The registry tries each language in order until one returns a suggestion.

#### 7.6 Fallback & Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("Repository path not found: {0}")]
    PathNotFound(PathBuf),

    #[error("Repository path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Scan timeout after {0}ms")]
    Timeout(u64),

    #[error("Filesystem error: {0}")]
    Filesystem(#[from] FSError),
}

/// Bootstrap result with fallback behavior
pub enum BootstrapResult {
    /// Full bootstrap completed successfully
    Complete(BootstrapContext),

    /// Partial bootstrap - some data available but scan incomplete
    Partial {
        context: BootstrapContext,
        reason: PartialReason,
    },

    /// Bootstrap failed entirely - pipeline continues without bootstrap
    Failed {
        error: BootstrapError,
    },
}

pub enum PartialReason {
    /// Hit max_files limit before completing scan
    MaxFilesReached { scanned: usize, limit: usize },

    /// Hit timeout before completing scan
    Timeout { duration_ms: u64 },

    /// Some manifests couldn't be read
    ManifestReadErrors { paths: Vec<String> },
}

impl BootstrapScanner {
    /// Scan with graceful degradation
    pub fn scan_with_fallback(&self, repo_path: &Path) -> BootstrapResult {
        match self.scan_internal(repo_path) {
            Ok(ctx) => BootstrapResult::Complete(ctx),
            Err(BootstrapError::Timeout(ms)) => {
                // Return partial results on timeout
                if let Some(partial) = self.get_partial_results() {
                    BootstrapResult::Partial {
                        context: partial,
                        reason: PartialReason::Timeout { duration_ms: ms },
                    }
                } else {
                    BootstrapResult::Failed {
                        error: BootstrapError::Timeout(ms)
                    }
                }
            }
            Err(e) => BootstrapResult::Failed { error: e },
        }
    }
}
```

#### 7.7 Pipeline Integration

```rust
impl AnalysisPipeline {
    pub async fn analyze(&mut self, progress: Option<&dyn ProgressHandler>) -> Result<UniversalBuild, AnalysisError> {
        // Step 1: Bootstrap scan
        progress.map(|p| p.on_bootstrap_started());

        let bootstrap_result = self.bootstrap_scanner.scan_with_fallback(&self.repo_path);

        let (bootstrap_context, bootstrap_quality) = match bootstrap_result {
            BootstrapResult::Complete(ctx) => {
                progress.map(|p| p.on_bootstrap_completed(&ctx, BootstrapQuality::Complete));
                (Some(ctx), BootstrapQuality::Complete)
            }
            BootstrapResult::Partial { context, reason } => {
                progress.map(|p| p.on_bootstrap_completed(&context, BootstrapQuality::Partial(reason.clone())));
                warn!("Bootstrap partial: {:?}", reason);
                (Some(context), BootstrapQuality::Partial(reason))
            }
            BootstrapResult::Failed { error } => {
                progress.map(|p| p.on_bootstrap_failed(&error));
                warn!("Bootstrap failed: {}, continuing without bootstrap context", error);
                (None, BootstrapQuality::None)
            }
        };

        // Step 2: Build system prompt with bootstrap context
        let system_prompt = self.build_system_prompt(bootstrap_context.as_ref(), bootstrap_quality);
        self.conversation.set_system_prompt(&system_prompt);

        // Step 3: Main analysis loop
        self.run_analysis_loop(progress).await
    }

    fn build_system_prompt(
        &self,
        bootstrap: Option<&BootstrapContext>,
        quality: BootstrapQuality,
    ) -> String {
        let mut prompt = BASE_SYSTEM_PROMPT.to_string();

        if let Some(ctx) = bootstrap {
            prompt.push_str("\n\n");
            prompt.push_str(&BootstrapScanner::format_for_prompt(ctx));

            // Add quality note
            match quality {
                BootstrapQuality::Complete => {
                    prompt.push_str("\n\nThis analysis is complete - you can rely on the suggestion.");
                }
                BootstrapQuality::Partial(reason) => {
                    prompt.push_str(&format!(
                        "\n\nNote: Pre-scan was partial ({:?}). Verify with tools if needed.",
                        reason
                    ));
                }
                BootstrapQuality::None => {}
            }

            // If we have a high-confidence suggestion, guide the LLM
            if let Some(ref suggestion) = ctx.suggestion {
                if suggestion.confidence >= 0.90 {
                    prompt.push_str(&format!(
                        "\n\nHigh confidence suggestion: {} + {} ({:.0}%). \
                         You may verify by reading the manifest, then submit detection.",
                        suggestion.language, suggestion.build_system, suggestion.confidence * 100.0
                    ));
                } else if suggestion.confidence >= 0.70 {
                    prompt.push_str(&format!(
                        "\n\nModerate confidence suggestion: {} + {} ({:.0}%). \
                         Please verify by examining relevant files.",
                        suggestion.language, suggestion.build_system, suggestion.confidence * 100.0
                    ));
                }
            }
        } else {
            // No bootstrap - LLM needs to explore from scratch
            prompt.push_str("\n\n");
            prompt.push_str("No pre-scan available. Please start by exploring the repository structure.");
        }

        prompt
    }
}
```

#### 7.8 Bootstrap Output Format

```rust
impl BootstrapScanner {
    pub fn format_for_prompt(ctx: &BootstrapContext) -> String {
        let mut output = String::new();

        // Header
        output.push_str("=== REPOSITORY PRE-SCAN ===\n\n");

        // Summary stats
        output.push_str(&format!(
            "Repository: {} files, {} directories, {}\n",
            ctx.summary.file_count,
            ctx.summary.dir_count,
            format_bytes(ctx.summary.total_size),
        ));

        // Languages (recognized)
        let lang_detection = &ctx.summary.language_detection;
        if !lang_detection.languages.is_empty() {
            output.push_str("Languages: ");
            let lang_strs: Vec<_> = lang_detection.languages.iter()
                .take(5)
                .map(|l| format!("{} ({:.0}%)", l.language, l.percentage))
                .collect();
            output.push_str(&lang_strs.join(", "));
            output.push('\n');
        }

        // Unknown extensions (top N most common)
        if !lang_detection.unknown_top.is_empty() {
            output.push_str("Other extensions (top): ");
            let ext_strs: Vec<_> = lang_detection.unknown_top.iter()
                .map(|e| format!(".{} ({})", e.extension, e.file_count))
                .collect();
            output.push_str(&ext_strs.join(", "));
            output.push('\n');
        }

        // Long tail summary (if many unknown extensions)
        if !lang_detection.unknown_tail.is_empty() {
            let tail_count = lang_detection.unknown_tail.len();
            let tail_files: usize = lang_detection.unknown_tail.iter().map(|e| e.file_count).sum();
            output.push_str(&format!(
                "Other extensions (long tail): {} more types, {} files total\n",
                tail_count, tail_files
            ));
            // Show a sample of the tail
            let sample: Vec<_> = lang_detection.unknown_tail.iter()
                .take(10)
                .map(|e| format!(".{}", e.extension))
                .collect();
            output.push_str(&format!("  Sample: {}\n", sample.join(", ")));
        }

        // Summary line for unknown content
        if lang_detection.unknown_count > 0 {
            let unknown_pct = (lang_detection.unknown_count as f32 /
                (lang_detection.recognized_count + lang_detection.unknown_count) as f32) * 100.0;
            if unknown_pct > 20.0 {
                output.push_str(&format!(
                    "Note: {:.0}% of files have unrecognized extensions - may be exotic/proprietary tech\n",
                    unknown_pct
                ));
            }
        }

        // Root files (important ones only)
        let important_root: Vec<_> = ctx.summary.root_files.iter()
            .filter(|f| is_important_root_file(f))
            .take(10)
            .collect();
        if !important_root.is_empty() {
            output.push_str(&format!("Root files: {}\n", important_root.join(", ")));
        }

        // Key directories
        if !ctx.summary.key_directories.is_empty() {
            output.push_str("Key directories: ");
            let dir_strs: Vec<_> = ctx.summary.key_directories.iter()
                .take(6)
                .map(|d| format!("{}/ ({:?})", d.path, d.purpose))
                .collect();
            output.push_str(&dir_strs.join(", "));
            output.push('\n');
        }

        // Workspace indicators
        if !ctx.summary.workspace_indicators.is_empty() {
            output.push_str(&format!(
                "Workspace: {} ({})\n",
                "Yes",
                ctx.summary.workspace_indicators.join(", ")
            ));
        }

        // Manifest files with contents
        output.push_str("\n--- MANIFEST FILES ---\n\n");
        for (i, manifest) in ctx.manifests.iter().enumerate() {
            output.push_str(&format!(
                "{}. {} ({}, {})\n",
                i + 1,
                manifest.path,
                manifest.manifest_type,
                format_bytes(manifest.size as u64),
            ));

            // Include truncated content for small files
            if manifest.size < 5000 {
                output.push_str("```\n");
                output.push_str(&manifest.content);
                if !manifest.content.ends_with('\n') {
                    output.push('\n');
                }
                output.push_str("```\n\n");
            } else {
                // For large files, show first 50 lines
                let preview: String = manifest.content
                    .lines()
                    .take(50)
                    .collect::<Vec<_>>()
                    .join("\n");
                output.push_str("```\n");
                output.push_str(&preview);
                output.push_str("\n... (truncated)\n```\n\n");
            }
        }

        // Build suggestion
        if let Some(ref suggestion) = ctx.suggestion {
            output.push_str("--- SUGGESTION ---\n\n");
            output.push_str(&format!(
                "Detected: {} + {}\n",
                suggestion.language, suggestion.build_system
            ));
            output.push_str(&format!("Confidence: {:.0}%\n", suggestion.confidence * 100.0));
            output.push_str(&format!("Reason: {}\n", suggestion.reasoning));
            if let Some(ref variant) = suggestion.variant {
                output.push_str(&format!("Variant: {}\n", variant));
            }
        }

        output
    }
}
```

#### 7.9 Configuration Options

```rust
pub struct BootstrapConfig {
    /// Max files to scan for stats (default: 5000)
    pub max_files_scan: usize,

    /// Max manifest file size to include full content (default: 50KB)
    pub max_manifest_size: usize,

    /// Max manifests to pre-read (default: 10)
    pub max_manifests: usize,

    /// Max scan duration before timeout (default: 5s)
    pub timeout_ms: u64,

    /// Max depth to scan for manifests (default: 5)
    pub max_depth: usize,

    /// Directories to skip entirely
    pub skip_dirs: Vec<String>,

    /// Whether to read manifest contents (default: true)
    pub read_contents: bool,

    /// Whether to generate build suggestion (default: true)
    pub generate_suggestion: bool,

    /// Number of top unknown extensions to show individually (default: 5)
    pub top_unknown_extensions: usize,

    /// Threshold (%) above which to warn about unrecognized extensions (default: 20)
    pub unknown_warning_threshold: f32,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            max_files_scan: 5000,
            max_manifest_size: 50_000,  // 50KB
            max_manifests: 10,
            timeout_ms: 5000,  // 5 seconds
            max_depth: 5,
            skip_dirs: vec![
                "node_modules".into(),
                ".git".into(),
                "target".into(),
                "vendor".into(),
                "__pycache__".into(),
                ".venv".into(),
                "venv".into(),
                "dist".into(),
                "build".into(),
                ".next".into(),
                ".nuxt".into(),
                "coverage".into(),
                ".cache".into(),
            ],
            read_contents: true,
            generate_suggestion: true,
            top_unknown_extensions: 5,
            unknown_warning_threshold: 20.0,
        }
    }
}
```

#### 7.10 Example Outputs

**Example 1: Simple Rust Project**

```
=== REPOSITORY PRE-SCAN ===

Repository: 47 files, 8 directories, 156.2 KB
Languages: Rust (89%), Shell (8%), TOML (3%)
Root files: Cargo.toml, Cargo.lock, README.md, .gitignore
Key directories: src/ (Source), tests/ (Tests)

--- MANIFEST FILES ---

1. Cargo.toml (1.2 KB, CargoToml)
```toml
[package]
name = "myapp"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
```

--- SUGGESTION ---

Detected: Rust + Cargo
Confidence: 95%
Reason: Cargo.toml at root
```

**Example 2: JavaScript Monorepo**

```
=== REPOSITORY PRE-SCAN ===

Repository: 2,847 files, 312 directories, 45.8 MB
Languages: TypeScript (72%), JavaScript (18%), JSON (8%)
Root files: package.json, pnpm-workspace.yaml, turbo.json, tsconfig.json
Key directories: packages/ (Workspace), apps/ (Workspace), libs/ (Source)
Workspace: Yes (pnpm-workspace.yaml, turbo.json)

--- MANIFEST FILES ---

1. package.json (2.1 KB, PackageJson)
```json
{
  "name": "mymonorepo",
  "private": true,
  "workspaces": ["packages/*", "apps/*"],
  "scripts": {
    "build": "turbo run build",
    "dev": "turbo run dev"
  }
}
```

2. pnpm-workspace.yaml (89 B, PnpmWorkspace)
```yaml
packages:
  - 'packages/*'
  - 'apps/*'
```

3. apps/web/package.json (1.5 KB, PackageJson)
```json
{
  "name": "@mymonorepo/web",
  "dependencies": {
    "next": "14.0.0"
  }
}
```

--- SUGGESTION ---

Detected: TypeScript + pnpm
Confidence: 90%
Reason: package.json at root (monorepo detected)
Variant: monorepo
```

**Example 3: Exotic/Unknown Project**

```
=== REPOSITORY PRE-SCAN ===

Repository: 234 files, 18 directories, 2.8 MB
Languages: Shell (12%)
Other extensions (top): .exs (89), .ex (67), .eex (34), .heex (23), .leex (12)
Other extensions (long tail): 8 more types, 45 files total
  Sample: .sface, .scss, .yml, .json, .md, .txt, .lock, .dockerfile
Note: 88% of files have unrecognized extensions - may be exotic/proprietary tech
Root files: mix.exs, mix.lock, README.md, .formatter.exs
Key directories: lib/ (Source), test/ (Tests), priv/ (Data), config/ (Config)

--- MANIFEST FILES ---

1. mix.exs (1.8 KB, Unknown)
```elixir
defmodule MyApp.MixProject do
  use Mix.Project

  def project do
    [
      app: :my_app,
      version: "0.1.0",
      elixir: "~> 1.14",
      deps: deps()
    ]
  end

  defp deps do
    [
      {:phoenix, "~> 1.7.0"},
      {:phoenix_live_view, "~> 0.18.0"}
    ]
  end
end
```

--- SUGGESTION ---

No confident suggestion - unrecognized build system.
Primary extensions: .exs, .ex (might be Elixir/Erlang ecosystem)
Recommendation: Use tools to explore mix.exs and lib/ structure.
```

**Example 4: Mixed/Polyglot Project**

```
=== REPOSITORY PRE-SCAN ===

Repository: 1,456 files, 89 directories, 12.4 MB
Languages: Python (34%), JavaScript (28%), Go (18%), Shell (8%)
Other extensions (top): .proto (45), .thrift (12), .avsc (8), .yaml (156)
Root files: Makefile, docker-compose.yml, README.md
Key directories: services/ (Workspace), proto/ (Data), scripts/ (Scripts)

--- MANIFEST FILES ---

1. Makefile (2.1 KB, Makefile)
```makefile
.PHONY: all build test

all: generate build

generate:
	protoc --go_out=. proto/*.proto
	python -m grpc_tools.protoc ...

build:
	cd services/api && go build
	cd services/worker && pip install -e .
	cd services/frontend && npm run build
```

2. services/api/go.mod (312 B, GoMod)
...
3. services/worker/pyproject.toml (890 B, PyProjectToml)
...
4. services/frontend/package.json (1.2 KB, PackageJson)
...

--- SUGGESTION ---

Detected: Polyglot + Makefile
Confidence: 65%
Reason: Multiple languages detected, Makefile coordinates builds
Note: This is a multi-service repository. Each service may need separate build detection.
```

**Example 5: Bootstrap Failed**

```
=== REPOSITORY PRE-SCAN ===

Pre-scan failed: Scan timeout after 5000ms
Reason: Repository too large to scan within timeout

Partial data available:
- Scanned 5000 of ~50000 files before timeout
- Found potential manifests: package.json (root)

Please explore the repository manually using tools.
```

### 7. LLM Tools (Enhanced)

Seven tools available to the LLM, enhanced from current implementation:

#### Tool 1: `list_files`

List directory contents with filtering.

```rust
pub struct ListFilesTool;

// Schema
{
    "path": {
        "type": "string",
        "description": "Directory path relative to repo root (empty or '.' for root)",
        "default": "."
    },
    "pattern": {
        "type": "string",
        "description": "Glob pattern to filter files (e.g., '*.rs', '*.{js,ts}')"
    },
    "recursive": {
        "type": "boolean",
        "description": "Include subdirectories",
        "default": false
    },
    "max_depth": {
        "type": "integer",
        "description": "Max depth when recursive (default: 3)",
        "default": 3
    },
    "include_hidden": {
        "type": "boolean",
        "description": "Include hidden files/directories",
        "default": false
    },
    "dirs_only": {
        "type": "boolean",
        "description": "Only return directories",
        "default": false
    },
    "files_only": {
        "type": "boolean",
        "description": "Only return files",
        "default": false
    }
}

// Output format
{
    "path": "src/",
    "entries": [
        {"name": "main.rs", "type": "file", "size": 1234},
        {"name": "lib.rs", "type": "file", "size": 567},
        {"name": "utils/", "type": "dir", "children": 5}
    ],
    "total": 3,
    "truncated": false
}
```

#### Tool 2: `read_file`

Read one or more files with smart truncation.

```rust
pub struct ReadFileTool;

// Schema
{
    "path": {
        "type": ["string", "array"],
        "description": "File path(s) relative to repo root. Can be single path or array of paths.",
        "items": {"type": "string"}
    },
    "start_line": {
        "type": "integer",
        "description": "Start reading from this line (1-indexed)",
        "default": 1
    },
    "max_lines": {
        "type": "integer",
        "description": "Maximum lines to return per file",
        "default": 500
    },
    "encoding": {
        "type": "string",
        "description": "File encoding (utf-8, latin-1, auto)",
        "default": "auto"
    }
}

// Output format (single file)
{
    "path": "Cargo.toml",
    "content": "...",
    "lines": 45,
    "truncated": false,
    "size": 1234
}

// Output format (multiple files)
{
    "files": [
        {"path": "Cargo.toml", "content": "...", "lines": 45},
        {"path": "src/main.rs", "content": "...", "lines": 120, "truncated": true}
    ],
    "total_files": 2,
    "errors": []
}
```

#### Tool 3: `search_files`

Find files by name pattern across repository.

```rust
pub struct SearchFilesTool;

// Schema
{
    "pattern": {
        "type": "string",
        "description": "Glob pattern for filename (e.g., '*.toml', 'package*.json', '**/test_*.py')"
    },
    "path": {
        "type": "string",
        "description": "Directory to search in (default: repo root)",
        "default": "."
    },
    "max_results": {
        "type": "integer",
        "description": "Maximum results to return",
        "default": 50
    },
    "include_hidden": {
        "type": "boolean",
        "description": "Search in hidden directories",
        "default": false
    }
}

// Output format
{
    "pattern": "*.toml",
    "matches": [
        {"path": "Cargo.toml", "size": 1234},
        {"path": "crates/cli/Cargo.toml", "size": 567}
    ],
    "total": 2,
    "truncated": false
}
```

#### Tool 4: `get_tree`

Get hierarchical view of repository structure.

```rust
pub struct GetTreeTool;

// Schema
{
    "path": {
        "type": "string",
        "description": "Root path for tree (default: repo root)",
        "default": "."
    },
    "max_depth": {
        "type": "integer",
        "description": "Maximum depth to traverse",
        "default": 4
    },
    "include_files": {
        "type": "boolean",
        "description": "Include files (not just directories)",
        "default": true
    },
    "include_hidden": {
        "type": "boolean",
        "description": "Include hidden files/directories",
        "default": false
    },
    "format": {
        "type": "string",
        "enum": ["ascii", "json"],
        "description": "Output format",
        "default": "ascii"
    }
}

// Output format (ascii)
.
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   └── utils/
│       ├── mod.rs
│       └── helpers.rs
├── tests/
│   └── integration.rs
└── README.md

// Output format (json)
{
    "name": ".",
    "type": "dir",
    "children": [
        {"name": "Cargo.toml", "type": "file", "size": 1234},
        {"name": "src", "type": "dir", "children": [...]}
    ]
}
```

#### Tool 5: `grep`

Search content across files.

```rust
pub struct GrepTool;

// Schema
{
    "pattern": {
        "type": "string",
        "description": "Regex pattern to search for"
    },
    "path": {
        "type": "string",
        "description": "Directory or file to search in",
        "default": "."
    },
    "file_pattern": {
        "type": "string",
        "description": "Only search files matching this glob (e.g., '*.rs')"
    },
    "case_insensitive": {
        "type": "boolean",
        "description": "Case insensitive search",
        "default": false
    },
    "context_lines": {
        "type": "integer",
        "description": "Lines of context before/after match",
        "default": 0
    },
    "max_matches": {
        "type": "integer",
        "description": "Maximum matches to return",
        "default": 50
    },
    "max_matches_per_file": {
        "type": "integer",
        "description": "Maximum matches per file",
        "default": 10
    }
}

// Output format
{
    "pattern": "fn main",
    "matches": [
        {
            "path": "src/main.rs",
            "line": 15,
            "content": "fn main() {",
            "context_before": ["", "// Entry point"],
            "context_after": ["    let args = Args::parse();"]
        }
    ],
    "total_matches": 1,
    "files_searched": 24,
    "truncated": false
}
```

#### Tool 6: `get_best_practices`

Get recommended build configuration template.

```rust
pub struct GetBestPracticesTool;

// Schema
{
    "language": {
        "type": "string",
        "description": "Programming language (rust, javascript, python, java, go, etc.)"
    },
    "build_system": {
        "type": "string",
        "description": "Build system (cargo, npm, yarn, pip, maven, gradle, etc.)"
    },
    "variant": {
        "type": "string",
        "description": "Specific variant (e.g., 'workspace' for Cargo, 'monorepo' for npm)"
    }
}

// Output: Best practice template with recommended:
// - Base images (build + runtime)
// - Build commands
// - Cache paths
// - Common artifacts
// - Typical ports
// - Health check patterns
```

#### Tool 7: `submit_detection`

Submit final detection result (terminal tool).

```rust
pub struct SubmitDetectionTool;

// Schema: UniversalBuild structure
{
    "version": "1.0",
    "metadata": {
        "project_name": "string",
        "language": "string",
        "build_system": "string",
        "confidence": "number (0-1)",
        "reasoning": "string"
    },
    "build": {
        "base_image": "string",
        "workdir": "string",
        "system_packages": ["string"],
        "environment": {"key": "value"},
        "pre_build_commands": ["string"],
        "build_commands": ["string"],
        "post_build_commands": ["string"],
        "cache_paths": ["string"],
        "artifacts": ["string"]
    },
    "runtime": {
        "base_image": "string",
        "workdir": "string",
        "system_packages": ["string"],
        "environment": {"key": "value"},
        "copy": [{"from": "string", "to": "string"}],
        "command": ["string"],
        "entrypoint": ["string"],
        "ports": [{"port": "number", "protocol": "string"}],
        "healthcheck": {
            "command": ["string"],
            "interval": "string",
            "timeout": "string",
            "retries": "number"
        }
    }
}
```

### 8. Tool Trait Definition

Each tool is a struct implementing a trait:

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> Value;

    fn execute(
        &self,
        args: &Value,
        fs: &dyn FileSystem,
        repo_path: &Path,
    ) -> Result<String, ToolError>;

    /// Whether this tool terminates the analysis
    fn is_terminal(&self) -> bool { false }

    /// Whether results can be cached
    fn is_cacheable(&self) -> bool { true }
}
```

### 9. FileSystem Trait

Synchronous file operations with advanced scanning capabilities.

```rust
pub trait FileSystem: Send + Sync {
    // Basic file operations
    fn read_file(&self, path: &Path) -> Result<String, FSError>;
    fn read_bytes(&self, path: &Path, max_bytes: usize) -> Result<Vec<u8>, FSError>;
    fn read_lines(&self, path: &Path, start: usize, count: usize) -> Result<Vec<String>, FSError>;
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn metadata(&self, path: &Path) -> Result<Metadata, FSError>;

    // Directory listing
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, FSError>;
    fn list_dir_filtered(&self, path: &Path, filter: &DirFilter) -> Result<Vec<DirEntry>, FSError>;

    // Advanced scanning
    fn walk(&self, path: &Path, options: &WalkOptions) -> Result<Vec<DirEntry>, FSError>;
    fn glob(&self, pattern: &str) -> Result<Vec<PathBuf>, FSError>;
    fn find_files(&self, query: &FileQuery) -> Result<Vec<PathBuf>, FSError>;

    // Content searching
    fn grep(&self, pattern: &str, options: &GrepOptions) -> Result<Vec<GrepMatch>, FSError>;
    fn grep_file(&self, path: &Path, pattern: &str) -> Result<Vec<LineMatch>, FSError>;

    // Tree representation
    fn tree(&self, path: &Path, options: &TreeOptions) -> Result<TreeNode, FSError>;
}

/// Directory entry with rich metadata
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub entry_type: EntryType,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

pub enum EntryType {
    File { is_binary: bool, is_executable: bool },
    Directory,
    Symlink { target: PathBuf },
}

pub struct Metadata {
    pub size: u64,
    pub is_binary: bool,
    pub is_executable: bool,
    pub modified: Option<SystemTime>,
    pub line_count: Option<usize>,  // For text files
}

/// Filter for directory listing
pub struct DirFilter {
    pub include_hidden: bool,
    pub include_dirs: bool,
    pub include_files: bool,
    pub extensions: Option<Vec<String>>,  // e.g., ["rs", "toml"]
    pub exclude_patterns: Vec<String>,     // e.g., ["node_modules", ".git"]
}

impl Default for DirFilter {
    fn default() -> Self {
        Self {
            include_hidden: false,
            include_dirs: true,
            include_files: true,
            extensions: None,
            exclude_patterns: vec![
                "node_modules".into(),
                ".git".into(),
                "target".into(),
                "__pycache__".into(),
                ".venv".into(),
                "vendor".into(),
            ],
        }
    }
}

/// Options for recursive directory walking
pub struct WalkOptions {
    pub max_depth: usize,
    pub filter: DirFilter,
    pub follow_symlinks: bool,
    pub max_files: usize,  // Safety limit
}

impl Default for WalkOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            filter: DirFilter::default(),
            follow_symlinks: false,
            max_files: 10_000,
        }
    }
}

/// Query for finding specific files
pub struct FileQuery {
    pub name_pattern: Option<String>,      // Glob pattern for filename
    pub path_pattern: Option<String>,      // Glob pattern for full path
    pub extensions: Option<Vec<String>>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub contains: Option<String>,          // File must contain this string
    pub max_results: usize,
}

/// Options for content searching
pub struct GrepOptions {
    pub path: Option<PathBuf>,             // Start path, defaults to root
    pub file_pattern: Option<String>,      // Only search matching files
    pub case_insensitive: bool,
    pub max_matches: usize,
    pub context_lines: usize,              // Lines before/after match
    pub include_binary: bool,
}

impl Default for GrepOptions {
    fn default() -> Self {
        Self {
            path: None,
            file_pattern: None,
            case_insensitive: false,
            max_matches: 100,
            context_lines: 0,
            include_binary: false,
        }
    }
}

/// A grep match with context
pub struct GrepMatch {
    pub path: PathBuf,
    pub matches: Vec<LineMatch>,
}

pub struct LineMatch {
    pub line_number: usize,
    pub content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// Options for tree generation
pub struct TreeOptions {
    pub max_depth: usize,
    pub include_files: bool,
    pub include_hidden: bool,
    pub exclude_patterns: Vec<String>,
    pub max_entries: usize,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            max_depth: 4,
            include_files: true,
            include_hidden: false,
            exclude_patterns: vec![
                "node_modules".into(),
                ".git".into(),
                "target".into(),
            ],
            max_entries: 500,
        }
    }
}

/// Tree node for hierarchical representation
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub entry_type: EntryType,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Render as ASCII tree
    pub fn to_string_tree(&self) -> String;

    /// Render as JSON
    pub fn to_json(&self) -> Value;
}
```

**Implementations:**

```rust
/// Real filesystem with path validation and security
pub struct RealFileSystem {
    root: PathBuf,
    config: FSConfig,
}

pub struct FSConfig {
    pub max_file_size: usize,      // Max bytes to read
    pub max_line_length: usize,    // Truncate long lines
    pub binary_detection_bytes: usize,
}

impl RealFileSystem {
    pub fn new(root: PathBuf) -> Result<Self, FSError>;
    pub fn with_config(root: PathBuf, config: FSConfig) -> Result<Self, FSError>;

    /// Validates path is within root (prevents path traversal)
    fn validate_path(&self, path: &Path) -> Result<PathBuf, FSError>;

    /// Detects if file is binary by sampling first N bytes
    fn is_binary_file(&self, path: &Path) -> Result<bool, FSError>;
}

/// In-memory filesystem for testing
pub struct MockFileSystem {
    files: HashMap<PathBuf, MockFile>,
    root: PathBuf,
}

pub struct MockFile {
    pub content: Vec<u8>,
    pub is_binary: bool,
    pub modified: SystemTime,
}

impl MockFileSystem {
    pub fn new() -> Self;

    pub fn with_root(root: PathBuf) -> Self;

    pub fn add_file(&mut self, path: impl AsRef<Path>, content: impl Into<Vec<u8>>);

    pub fn add_text_file(&mut self, path: impl AsRef<Path>, content: &str);

    pub fn add_binary_file(&mut self, path: impl AsRef<Path>, content: Vec<u8>);

    /// Create directory structure from a map
    pub fn from_tree(tree: HashMap<&str, &str>) -> Self;
}
```

### 10. Validator

Validates UniversalBuild results with composable rules.

```rust
pub struct Validator {
    rules: Vec<Box<dyn ValidationRule>>,
}

impl Validator {
    pub fn new() -> Self;
    pub fn with_rule(mut self, rule: impl ValidationRule + 'static) -> Self;

    pub fn validate(&self, build: &UniversalBuild) -> Result<(), ValidationError>;
}

pub trait ValidationRule: Send + Sync {
    fn name(&self) -> &str;
    fn validate(&self, build: &UniversalBuild) -> Result<(), String>;
}

// Built-in rules
pub struct RequiredFieldsRule;
pub struct NonEmptyCommandsRule;
pub struct ValidImageNameRule;
pub struct ConfidenceRangeRule;
pub struct PathsExistRule;  // Validates artifact paths make sense
```

### 11. Progress Reporting

Trait with granular methods for detailed progress tracking:

```rust
/// Progress handler with granular event methods
///
/// All methods have default no-op implementations, so handlers can
/// override only the events they care about.
pub trait ProgressHandler: Send + Sync {
    // === Lifecycle Events ===

    /// Analysis has started
    fn on_started(&self, _repo_path: &Path) {}

    /// Analysis completed successfully
    fn on_completed(&self, _summary: &AnalysisSummary) {}

    /// Analysis failed with error
    fn on_failed(&self, _error: &AnalysisError) {}

    // === Jumpstart Events ===

    /// Jumpstart scan started
    fn on_jumpstart_started(&self) {}

    /// Jumpstart scan completed
    fn on_jumpstart_completed(&self, _manifests: usize, _duration_ms: u64) {}

    // === Iteration Events ===

    /// New iteration started
    fn on_iteration_started(&self, _iteration: usize) {}

    /// Iteration completed
    fn on_iteration_completed(&self, _iteration: usize, _tool_calls: usize, _duration_ms: u64) {}

    // === LLM Events ===

    /// LLM request started
    fn on_llm_request_started(&self, _message_count: usize) {}

    /// LLM response received
    fn on_llm_response_received(&self, _response: &LLMResponseSummary) {}

    /// LLM request failed (may retry)
    fn on_llm_request_failed(&self, _error: &LLMError, _will_retry: bool) {}

    // === Tool Events ===

    /// Tool execution started
    fn on_tool_started(&self, _name: &str, _args: &Value) {}

    /// Tool execution completed
    fn on_tool_completed(&self, _name: &str, _result: &ToolResultSummary) {}

    /// Tool result served from cache
    fn on_tool_cached(&self, _name: &str) {}

    /// Tool execution failed
    fn on_tool_failed(&self, _name: &str, _error: &str) {}

    // === Validation Events ===

    /// Validation started
    fn on_validation_started(&self) {}

    /// Validation passed
    fn on_validation_passed(&self, _confidence: f64) {}

    /// Validation failed (will send feedback to LLM)
    fn on_validation_failed(&self, _errors: &[String]) {}

    // === Debug/Verbose Events ===

    /// Raw message added to conversation (for debugging)
    fn on_message_added(&self, _role: &str, _preview: &str) {}

    /// Cache statistics updated
    fn on_cache_stats(&self, _stats: &CacheStats) {}
}

/// Summary of LLM response
pub struct LLMResponseSummary {
    pub has_content: bool,
    pub tool_call_count: usize,
    pub token_usage: Option<TokenUsage>,
    pub duration_ms: u64,
}

pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Summary of tool execution result
pub struct ToolResultSummary {
    pub success: bool,
    pub output_bytes: usize,
    pub duration_ms: u64,
    pub truncated: bool,
}

/// Summary of completed analysis
pub struct AnalysisSummary {
    pub language: String,
    pub build_system: String,
    pub confidence: f64,
    pub iterations: usize,
    pub total_tool_calls: usize,
    pub cached_tool_calls: usize,
    pub total_duration_ms: u64,
    pub llm_duration_ms: u64,
    pub tool_duration_ms: u64,
}

/// Cache statistics
pub struct CacheStats {
    pub hits: usize,
    pub misses: usize,
    pub entries: usize,
    pub bytes: usize,
}

// === Handler Implementations ===

/// No-op handler (default)
pub struct NoOpHandler;

impl ProgressHandler for NoOpHandler {}

/// Logging handler - logs events at appropriate levels
pub struct LoggingHandler {
    level: LogLevel,
}

pub enum LogLevel {
    Quiet,      // Only errors
    Normal,     // Lifecycle + summary
    Verbose,    // + iterations + tools
    Debug,      // Everything
}

impl ProgressHandler for LoggingHandler {
    fn on_started(&self, repo_path: &Path) {
        if matches!(self.level, LogLevel::Normal | LogLevel::Verbose | LogLevel::Debug) {
            info!("Starting analysis: {}", repo_path.display());
        }
    }

    fn on_tool_started(&self, name: &str, args: &Value) {
        if matches!(self.level, LogLevel::Verbose | LogLevel::Debug) {
            debug!("Tool: {} {:?}", name, args);
        }
    }

    // ... other methods
}

/// Metrics collector - aggregates statistics
pub struct MetricsCollector {
    metrics: Mutex<AnalysisMetrics>,
}

pub struct AnalysisMetrics {
    pub tool_calls: HashMap<String, ToolMetrics>,
    pub llm_calls: usize,
    pub total_tokens: usize,
    pub cache_hit_rate: f64,
}

pub struct ToolMetrics {
    pub call_count: usize,
    pub total_duration_ms: u64,
    pub cache_hits: usize,
    pub errors: usize,
}

impl ProgressHandler for MetricsCollector {
    fn on_tool_completed(&self, name: &str, result: &ToolResultSummary) {
        let mut metrics = self.metrics.lock().unwrap();
        let tool = metrics.tool_calls.entry(name.to_string()).or_default();
        tool.call_count += 1;
        tool.total_duration_ms += result.duration_ms;
    }

    // ... other methods
}

/// Callback wrapper - for simple closure-based handling
pub struct CallbackHandler<F> {
    callback: F,
}

impl<F> CallbackHandler<F>
where
    F: Fn(ProgressEvent) + Send + Sync,
{
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

/// Unified event enum for callback-style handlers
pub enum ProgressEvent {
    Started { repo_path: PathBuf },
    Completed { summary: AnalysisSummary },
    Failed { error: String },
    JumpstartCompleted { manifests: usize, duration_ms: u64 },
    IterationStarted { iteration: usize },
    IterationCompleted { iteration: usize, tool_calls: usize },
    LLMRequestStarted,
    LLMResponseReceived { tool_calls: usize },
    ToolStarted { name: String, args: Value },
    ToolCompleted { name: String, success: bool, duration_ms: u64 },
    ToolCached { name: String },
    ValidationPassed { confidence: f64 },
    ValidationFailed { errors: Vec<String> },
}

impl<F: Fn(ProgressEvent) + Send + Sync> ProgressHandler for CallbackHandler<F> {
    fn on_started(&self, repo_path: &Path) {
        (self.callback)(ProgressEvent::Started {
            repo_path: repo_path.to_path_buf(),
        });
    }

    fn on_tool_started(&self, name: &str, args: &Value) {
        (self.callback)(ProgressEvent::ToolStarted {
            name: name.to_string(),
            args: args.clone(),
        });
    }

    // ... other methods dispatch to callback
}

/// Composite handler - forwards to multiple handlers
pub struct CompositeHandler {
    handlers: Vec<Box<dyn ProgressHandler>>,
}

impl CompositeHandler {
    pub fn new() -> Self {
        Self { handlers: vec![] }
    }

    pub fn add(mut self, handler: impl ProgressHandler + 'static) -> Self {
        self.handlers.push(Box::new(handler));
        self
    }
}

impl ProgressHandler for CompositeHandler {
    fn on_started(&self, repo_path: &Path) {
        for h in &self.handlers {
            h.on_started(repo_path);
        }
    }

    // ... other methods forward to all handlers
}
```

### 12. Error Types

Structured errors with context:

```rust
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("LLM error: {0}")]
    LLM(#[from] LLMError),

    #[error("Tool error in {tool}: {message}")]
    Tool { tool: String, message: String },

    #[error("Validation failed: {0}")]
    Validation(#[from] ValidationError),

    #[error("Max iterations ({max}) exceeded without result")]
    MaxIterations { max: usize },

    #[error("Analysis timeout after {seconds}s")]
    Timeout { seconds: u64 },

    #[error("Repository error: {0}")]
    Repository(String),
}

#[derive(Debug, thiserror::Error)]
pub enum LLMError {
    #[error("API error: {message}")]
    Api { message: String, status: Option<u16> },

    #[error("Authentication failed")]
    Auth,

    #[error("Request timeout after {seconds}s")]
    Timeout { seconds: u64 },

    #[error("Rate limited, retry after {retry_after:?}s")]
    RateLimit { retry_after: Option<u64> },

    #[error("Invalid response: {message}")]
    InvalidResponse { message: String },

    #[error("Network error: {0}")]
    Network(String),
}
```

## Directory Structure

```
src/
├── lib.rs
├── main.rs
├── config.rs
│
├── pipeline/                    # Analysis orchestration
│   ├── mod.rs
│   ├── context.rs               # PipelineContext (owns dependencies)
│   ├── factory.rs               # PipelineFactory
│   ├── pipeline.rs              # AnalysisPipeline
│   ├── config.rs                # PipelineConfig
│   └── error.rs                 # AnalysisError
│
├── progress/                    # Progress reporting
│   ├── mod.rs
│   ├── handler.rs               # ProgressHandler trait
│   ├── events.rs                # ProgressEvent enum
│   ├── summary.rs               # AnalysisSummary, ToolResultSummary
│   ├── logging.rs               # LoggingHandler
│   ├── metrics.rs               # MetricsCollector
│   └── composite.rs             # CompositeHandler
│
├── conversation/                # LLM conversation management
│   ├── mod.rs
│   ├── manager.rs               # ConversationManager
│   ├── messages.rs              # Message, MessageContent
│   ├── options.rs               # ChatOptions
│   └── prompt.rs                # System prompts
│
├── llm/                         # LLM client abstraction
│   ├── mod.rs
│   ├── client.rs                # LLMClient trait
│   ├── genai.rs                 # GenAIClient implementation
│   ├── mock.rs                  # MockLLMClient for testing
│   ├── response.rs              # LLMResponse, ToolCall
│   └── error.rs                 # LLMError
│
├── tools/                       # Tool system
│   ├── mod.rs
│   ├── system.rs                # ToolSystem facade
│   ├── registry.rs              # ToolRegistry
│   ├── executor.rs              # ToolExecutor
│   ├── cache.rs                 # ToolCache
│   ├── trait.rs                 # Tool trait
│   ├── error.rs                 # ToolError
│   └── defs/                    # Tool implementations
│       ├── mod.rs               # Exports all tools
│       ├── list_files.rs
│       ├── read_file.rs
│       ├── search_files.rs
│       ├── file_tree.rs
│       ├── grep.rs
│       ├── best_practices.rs    # Uses LanguageRegistry
│       └── submit.rs
│
├── languages/                   # Language definitions (extensions, manifests, detection, templates)
│   ├── mod.rs                   # LanguageRegistry + LanguageDefinition trait
│   ├── rust.rs                  # Rust/Cargo
│   ├── java.rs                  # Java (Maven, Gradle)
│   ├── kotlin.rs                # Kotlin (Gradle)
│   ├── javascript.rs            # JavaScript (npm, yarn, pnpm, bun)
│   ├── typescript.rs            # TypeScript
│   ├── python.rs                # Python (pip, poetry, pipenv)
│   ├── go.rs                    # Go modules
│   ├── dotnet.rs                # .NET SDK
│   ├── ruby.rs                  # Ruby (Bundler)
│   ├── php.rs                   # PHP (Composer)
│   ├── cpp.rs                   # C/C++ (CMake, Make)
│   └── elixir.rs                # Elixir (Mix)
│
├── fs/                          # Filesystem abstraction
│   ├── mod.rs
│   ├── trait.rs                 # FileSystem trait
│   ├── real.rs                  # RealFileSystem
│   └── mock.rs                  # MockFileSystem
│
├── validation/                  # Validation system
│   ├── mod.rs
│   ├── validator.rs             # Validator
│   ├── rules.rs                 # ValidationRule trait + implementations
│   └── error.rs                 # ValidationError
│
├── bootstrap/                   # Repository bootstrap system
│   ├── mod.rs
│   ├── scanner.rs               # BootstrapScanner (uses LanguageRegistry)
│   └── context.rs               # BootstrapContext, RepoSummary, LanguageDetection
│
├── detection/                   # Public service layer
│   ├── mod.rs
│   └── service.rs               # DetectionService
│
├── output/                      # Output formats
│   ├── mod.rs
│   ├── schema.rs                # UniversalBuild
│   └── dockerfile.rs            # Dockerfile generation
│
└── cli/                         # CLI interface
    ├── mod.rs
    ├── commands.rs
    └── output.rs
```

## Key Design Decisions

### Decision 1: Dependency injection via PipelineContext

**Choice:** Long-lived dependencies owned by `PipelineContext`, borrowed by pipeline.

**Rationale:**
- LLMClient is expensive to create (model validation, connection setup)
- Clear ownership model - context lives as long as service
- Easy to inject mocks for testing
- No Arc/Mutex complexity for single-threaded use

### Decision 2: Synchronous FileSystem trait

**Choice:** FileSystem methods are synchronous (not async).

**Rationale:**
- Local filesystem operations are fast, async overhead not worth it
- Simpler trait, simpler implementations
- Tools are already behind async boundary in pipeline
- Can wrap in `spawn_blocking` if ever needed

### Decision 3: Tool trait with schema method

**Choice:** Each tool implements a trait that returns its JSON schema.

**Rationale:**
- Schema co-located with implementation
- Type-safe tool registration
- Easy to add new tools (implement trait, add to registry)
- Schema can reference implementation constants

### Decision 4: Validation via composable rules

**Choice:** Validator holds a list of `ValidationRule` trait objects.

**Rationale:**
- Easy to add/remove rules
- Each rule is independently testable
- Rules can be configured per-use-case
- Error messages are rule-specific

### Decision 5: Progress via trait not callback

**Choice:** `ProgressHandler` trait instead of `Fn` callback.

**Rationale:**
- Can have stateful handlers (e.g., aggregate stats)
- `NoOpHandler` avoids Option checks everywhere
- Trait objects are more flexible than closures
- Still simple - one method to implement

## Example Usage

### Production Use

```rust
// In DetectionService::new()
let context = PipelineContext::new(&config)?;

// In DetectionService::detect()
let mut pipeline = PipelineFactory::create(&self.context, repo_path)?;
let result = pipeline.analyze(jumpstart_ctx, Some(&self.progress_handler)).await?;
```

### Testing

```rust
#[tokio::test]
async fn test_rust_project_detection() {
    // Setup mock filesystem
    let mut fs = MockFileSystem::new();
    fs.add_file("Cargo.toml", r#"[package]\nname = "test""#);
    fs.add_file("src/main.rs", "fn main() {}");

    // Setup mock LLM with scripted responses
    let mut llm = MockLLMClient::new();
    llm.expect_response(LLMResponse {
        tool_calls: vec![ToolCall {
            name: "get_file_tree".into(),
            args: json!({}),
            call_id: "1".into(),
        }],
        ..Default::default()
    });
    llm.expect_response(LLMResponse {
        tool_calls: vec![ToolCall {
            name: "read_file".into(),
            args: json!({"path": "Cargo.toml"}),
            call_id: "2".into(),
        }],
        ..Default::default()
    });
    llm.expect_response(LLMResponse {
        tool_calls: vec![ToolCall {
            name: "submit_detection".into(),
            args: json!({
                "version": "1.0",
                "metadata": { "language": "rust", "build_system": "cargo", "confidence": 0.95, "reasoning": "Found Cargo.toml" },
                "build": { /* ... */ },
                "runtime": { /* ... */ }
            }),
            call_id: "3".into(),
        }],
        ..Default::default()
    });

    // Create context with mocks
    let context = PipelineContext::with_mocks(Box::new(llm), Box::new(fs));

    // Run analysis
    let mut pipeline = PipelineFactory::create(&context, PathBuf::from("/repo"))?;
    let result = pipeline.analyze(None, None).await?;

    assert_eq!(result.metadata.language, "rust");
    assert_eq!(result.metadata.build_system, "cargo");
}
```

## Migration Strategy

1. **Phase 1:** Create trait definitions (`LLMClient`, `FileSystem`, `Tool`, `ValidationRule`, `ProgressHandler`)
2. **Phase 2:** Implement `RealFileSystem` and `MockFileSystem`
3. **Phase 3:** Implement `GenAIClient` and `MockLLMClient`
4. **Phase 4:** Create tool implementations with new trait
5. **Phase 5:** Build `ToolSystem`, `ToolRegistry`, `ToolExecutor`, `ToolCache`
6. **Phase 6:** Build `Validator` with rules
7. **Phase 7:** Build `ConversationManager`
8. **Phase 8:** Build `PipelineContext` and `PipelineFactory`
9. **Phase 9:** Build `AnalysisPipeline`
10. **Phase 10:** Wire `DetectionService` to new pipeline
11. **Phase 11:** Remove old implementation, update exports

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Lifetime complexity | Keep lifetimes simple - context outlives pipeline |
| Breaking changes during migration | Keep old code until new pipeline fully works |
| Performance regression | Benchmark before/after, minimal abstraction overhead |
| Over-engineering | Start with minimal trait surface, extend as needed |

## Resolved Questions

1. ~~Should `ToolSystem` be passed by reference or owned?~~ → **Owned per-analysis, borrows FileSystem**
2. ~~Async for FileSystem?~~ → **No, sync is sufficient for local FS**
3. ~~Validation error suggestions?~~ → **Yes, via rule-specific error messages**

---

## 10. Test Infrastructure

### 10.1 Test Fixture Repositories

Minimal test repositories covering common project types. Each fixture is a real directory structure that can be analyzed.

```
tests/
├── fixtures/
│   ├── single-language/
│   │   ├── rust-cargo/           # Minimal Cargo.toml + src/main.rs
│   │   ├── rust-workspace/       # Cargo workspace with multiple crates
│   │   ├── node-npm/             # package.json + index.js
│   │   ├── node-yarn/            # package.json + yarn.lock
│   │   ├── node-pnpm/            # package.json + pnpm-lock.yaml
│   │   ├── node-bun/             # package.json + bun.lockb
│   │   ├── python-pip/           # requirements.txt + main.py
│   │   ├── python-poetry/        # pyproject.toml (poetry)
│   │   ├── python-pipenv/        # Pipfile + Pipfile.lock
│   │   ├── java-maven/           # pom.xml + src/main/java/
│   │   ├── java-gradle/          # build.gradle + src/main/java/
│   │   ├── kotlin-gradle/        # build.gradle.kts + src/main/kotlin/
│   │   ├── go-mod/               # go.mod + main.go
│   │   ├── dotnet-csproj/        # *.csproj + Program.cs
│   │   ├── ruby-bundler/         # Gemfile + app.rb
│   │   ├── php-composer/         # composer.json + index.php
│   │   ├── elixir-mix/           # mix.exs + lib/
│   │   └── cpp-cmake/            # CMakeLists.txt + main.cpp
│   │
│   ├── monorepos/
│   │   ├── npm-workspaces/       # Root package.json with workspaces
│   │   │   ├── package.json      # "workspaces": ["packages/*"]
│   │   │   └── packages/
│   │   │       ├── web/          # React app
│   │   │       ├── api/          # Express server
│   │   │       └── shared/       # Shared types
│   │   │
│   │   ├── turborepo/            # Turborepo monorepo
│   │   │   ├── turbo.json
│   │   │   ├── package.json
│   │   │   └── apps/
│   │   │       ├── web/
│   │   │       └── docs/
│   │   │
│   │   ├── cargo-workspace/      # Rust workspace
│   │   │   ├── Cargo.toml        # [workspace] members
│   │   │   ├── crates/
│   │   │   │   ├── core/
│   │   │   │   └── cli/
│   │   │   └── apps/
│   │   │       └── server/
│   │   │
│   │   ├── gradle-multiproject/  # Gradle multi-module
│   │   │   ├── settings.gradle
│   │   │   ├── build.gradle
│   │   │   └── modules/
│   │   │       ├── api/
│   │   │       └── core/
│   │   │
│   │   ├── maven-multimodule/    # Maven multi-module
│   │   │   ├── pom.xml           # Parent POM with modules
│   │   │   └── modules/
│   │   │       ├── core/
│   │   │       └── web/
│   │   │
│   │   └── polyglot/             # Mixed languages
│   │       ├── backend/          # Rust API
│   │       │   └── Cargo.toml
│   │       ├── frontend/         # React app
│   │       │   └── package.json
│   │       └── scripts/          # Python tooling
│   │           └── requirements.txt
│   │
│   ├── edge-cases/
│   │   ├── empty-repo/           # Just .gitignore
│   │   ├── no-manifest/          # Source files but no manifest
│   │   ├── multiple-manifests/   # Cargo.toml + package.json in same dir
│   │   ├── nested-projects/      # Projects inside projects
│   │   ├── vendor-heavy/         # Large vendor/node_modules (should ignore)
│   │   ├── generated-code/       # Lots of generated files
│   │   └── exotic-language/      # Uncommon language (Zig, Nim, etc.)
│   │
│   └── expected/                 # Expected UniversalBuild outputs
│       ├── rust-cargo.json
│       ├── node-npm.json
│       └── ...
```

### 10.2 LLM Recording/Replay System

Cache LLM request-response pairs for deterministic testing. Re-query LLM only when request changes.

```rust
/// Records and replays LLM conversations for testing
pub struct RecordingLLMClient {
    inner: Box<dyn LLMClient>,
    recordings_dir: PathBuf,
    mode: RecordingMode,
}

pub enum RecordingMode {
    /// Record all interactions, overwrite existing
    Record,
    /// Replay from cache, fail if not found
    Replay,
    /// Replay if cached, record if not (default for tests)
    Auto,
}

/// A single recorded exchange
#[derive(Serialize, Deserialize)]
pub struct RecordedExchange {
    /// Hash of the request (messages + tools + config)
    request_hash: String,
    /// The request that was sent
    request: RecordedRequest,
    /// The response that was received
    response: LLMResponse,
    /// When this was recorded
    recorded_at: DateTime<Utc>,
    /// Model used for recording
    model: String,
}

#[derive(Serialize, Deserialize)]
pub struct RecordedRequest {
    messages: Vec<Message>,
    tools: Vec<ToolDefinition>,
    temperature: Option<f32>,
    max_tokens: Option<usize>,
}

impl RecordingLLMClient {
    pub fn new(inner: Box<dyn LLMClient>, recordings_dir: PathBuf) -> Self {
        Self {
            inner,
            recordings_dir,
            mode: RecordingMode::Auto,
        }
    }

    fn request_hash(&self, request: &RecordedRequest) -> String {
        // Hash messages content + tool definitions (ignore timestamps, IDs)
        let canonical = serde_json::to_string(&request).unwrap();
        format!("{:x}", md5::compute(canonical))
    }

    fn recording_path(&self, hash: &str) -> PathBuf {
        self.recordings_dir.join(format!("{}.json", hash))
    }
}

#[async_trait]
impl LLMClient for RecordingLLMClient {
    async fn chat(&self, request: ChatRequest) -> Result<LLMResponse> {
        let recorded_request = RecordedRequest {
            messages: request.messages.clone(),
            tools: request.tools.clone(),
            temperature: request.temperature,
            max_tokens: request.max_tokens,
        };
        let hash = self.request_hash(&recorded_request);
        let path = self.recording_path(&hash);

        // Check for existing recording
        if path.exists() && matches!(self.mode, RecordingMode::Replay | RecordingMode::Auto) {
            let exchange: RecordedExchange = serde_json::from_str(
                &std::fs::read_to_string(&path)?
            )?;
            return Ok(exchange.response);
        }

        // No recording found
        if matches!(self.mode, RecordingMode::Replay) {
            anyhow::bail!("No recording found for request hash: {}", hash);
        }

        // Record new response
        let response = self.inner.chat(request).await?;

        let exchange = RecordedExchange {
            request_hash: hash.clone(),
            request: recorded_request,
            response: response.clone(),
            recorded_at: Utc::now(),
            model: self.inner.model_name().to_string(),
        };

        std::fs::create_dir_all(&self.recordings_dir)?;
        std::fs::write(&path, serde_json::to_string_pretty(&exchange)?)?;

        Ok(response)
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}
```

#### Recording Directory Structure

```
tests/
└── recordings/
    ├── rust-cargo/
    │   ├── a1b2c3d4.json    # First exchange (list_files)
    │   ├── e5f6g7h8.json    # Second exchange (read_file Cargo.toml)
    │   └── i9j0k1l2.json    # Final exchange (submit_detection)
    ├── node-npm/
    │   └── ...
    └── monorepo-turborepo/
        └── ...
```

#### Recording File Format

```json
{
  "request_hash": "a1b2c3d4e5f6g7h8",
  "request": {
    "messages": [
      {"role": "system", "content": "You are analyzing..."},
      {"role": "user", "content": "Analyze this repository..."}
    ],
    "tools": [
      {"name": "list_files", "description": "...", "parameters": {...}}
    ],
    "temperature": 0.0,
    "max_tokens": 8192
  },
  "response": {
    "content": null,
    "tool_calls": [
      {"name": "list_files", "args": {"path": "."}, "call_id": "1"}
    ],
    "usage": {"prompt_tokens": 1234, "completion_tokens": 56}
  },
  "recorded_at": "2024-01-15T10:30:00Z",
  "model": "qwen2.5-coder:7b"
}
```

### 10.3 Monorepo Detection Strategy

Monorepos require special handling to detect multiple build targets.

#### Detection Signals

| Signal | Indicates |
|--------|-----------|
| `package.json` with `"workspaces"` | npm/yarn/pnpm workspaces |
| `turbo.json` | Turborepo |
| `pnpm-workspace.yaml` | pnpm workspaces |
| `lerna.json` | Lerna monorepo |
| `Cargo.toml` with `[workspace]` | Cargo workspace |
| `settings.gradle` with `include` | Gradle multi-project |
| Parent `pom.xml` with `<modules>` | Maven multi-module |
| `nx.json` | Nx monorepo |
| `rush.json` | Rush monorepo |

#### Monorepo Output Format

For monorepos, `UniversalBuild` should return multiple targets:

```rust
pub struct UniversalBuild {
    pub version: String,
    pub metadata: BuildMetadata,
    pub build: BuildStage,
    pub runtime: RuntimeStage,
    /// For monorepos: individual project builds
    pub projects: Option<Vec<ProjectBuild>>,
}

pub struct ProjectBuild {
    /// Relative path from repo root
    pub path: String,
    /// Project name (from manifest)
    pub name: String,
    /// Project-specific build configuration
    pub build: BuildStage,
    pub runtime: RuntimeStage,
}
```

#### Example: Turborepo Monorepo

Input structure:
```
/
├── turbo.json
├── package.json
├── apps/
│   ├── web/
│   │   └── package.json    # Next.js app
│   └── api/
│       └── package.json    # Express server
└── packages/
    └── ui/
        └── package.json    # Shared components
```

Expected output:
```json
{
  "version": "1.0",
  "metadata": {
    "language": "typescript",
    "build_system": "turborepo",
    "confidence": 0.92,
    "reasoning": "Found turbo.json with apps/web, apps/api, packages/ui"
  },
  "build": {
    "base_image": "node:20-alpine",
    "commands": ["npm ci", "npx turbo build"]
  },
  "projects": [
    {
      "path": "apps/web",
      "name": "web",
      "build": {
        "commands": ["npm run build"],
        "artifacts": [".next/"]
      },
      "runtime": {
        "command": ["npm", "start"],
        "ports": [3000]
      }
    },
    {
      "path": "apps/api",
      "name": "api",
      "build": {
        "commands": ["npm run build"],
        "artifacts": ["dist/"]
      },
      "runtime": {
        "command": ["node", "dist/index.js"],
        "ports": [4000]
      }
    }
  ]
}
```

### 10.4 E2E Test Scenarios

```rust
/// E2E test that runs full detection against fixture repositories
#[tokio::test]
async fn e2e_rust_cargo_detection() {
    let fixture_path = PathBuf::from("tests/fixtures/single-language/rust-cargo");
    let expected_path = PathBuf::from("tests/fixtures/expected/rust-cargo.json");

    // Use recording client for deterministic results
    let llm = RecordingLLMClient::new(
        create_embedded_client().await.unwrap(),
        PathBuf::from("tests/recordings/rust-cargo"),
    );

    let result = run_detection(fixture_path, Box::new(llm)).await.unwrap();
    let expected: UniversalBuild = serde_json::from_str(
        &std::fs::read_to_string(expected_path).unwrap()
    ).unwrap();

    assert_eq!(result.metadata.language, expected.metadata.language);
    assert_eq!(result.metadata.build_system, expected.metadata.build_system);
    assert!(result.metadata.confidence >= 0.8);
}

/// Test monorepo detection
#[tokio::test]
async fn e2e_turborepo_monorepo() {
    let fixture_path = PathBuf::from("tests/fixtures/monorepos/turborepo");

    let llm = RecordingLLMClient::new(
        create_embedded_client().await.unwrap(),
        PathBuf::from("tests/recordings/monorepo-turborepo"),
    );

    let result = run_detection(fixture_path, Box::new(llm)).await.unwrap();

    assert_eq!(result.metadata.build_system, "turborepo");
    assert!(result.projects.is_some());

    let projects = result.projects.unwrap();
    assert!(projects.iter().any(|p| p.path == "apps/web"));
    assert!(projects.iter().any(|p| p.path == "apps/api"));
}

/// Test edge case: empty repository
#[tokio::test]
async fn e2e_empty_repo() {
    let fixture_path = PathBuf::from("tests/fixtures/edge-cases/empty-repo");

    let result = run_detection(fixture_path, create_mock_llm()).await;

    // Should return low confidence or error
    match result {
        Ok(build) => assert!(build.metadata.confidence < 0.5),
        Err(e) => assert!(e.to_string().contains("no build system")),
    }
}

/// Test edge case: multiple manifests
#[tokio::test]
async fn e2e_multiple_manifests() {
    let fixture_path = PathBuf::from("tests/fixtures/edge-cases/multiple-manifests");

    let llm = RecordingLLMClient::new(
        create_embedded_client().await.unwrap(),
        PathBuf::from("tests/recordings/multiple-manifests"),
    );

    let result = run_detection(fixture_path, Box::new(llm)).await.unwrap();

    // Should pick primary language or report as polyglot
    assert!(result.metadata.confidence >= 0.7);
}

/// Performance test: ensure detection completes within timeout
#[tokio::test]
async fn e2e_performance_large_monorepo() {
    let fixture_path = PathBuf::from("tests/fixtures/monorepos/polyglot");

    let start = std::time::Instant::now();
    let result = run_detection(fixture_path, create_recording_llm()).await;
    let duration = start.elapsed();

    assert!(result.is_ok());
    assert!(duration < std::time::Duration::from_secs(60),
        "Detection took too long: {:?}", duration);
}
```

### 10.5 CI Test Matrix

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --lib

  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run integration tests with recordings
        run: cargo test --test '*' -- --test-threads=1
        env:
          AIPACK_RECORDING_MODE: replay

  e2e-record:
    # Only run on main branch to update recordings
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Download embedded model
        run: cargo run -- download-model --model qwen2.5-coder:1.5b
      - name: Record E2E tests
        run: cargo test --test e2e -- --test-threads=1
        env:
          AIPACK_RECORDING_MODE: record
      - name: Commit updated recordings
        run: |
          git config user.name "CI Bot"
          git config user.email "ci@example.com"
          git add tests/recordings/
          git diff --staged --quiet || git commit -m "chore: Update LLM recordings"
          git push
```

### 10.6 Test Utilities

```rust
/// Helper to create fixture directory structure programmatically
pub fn create_fixture(name: &str, files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, content) in files {
        let file_path = dir.path().join(path);
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(file_path, content).unwrap();
    }
    dir
}

/// Create minimal Rust project fixture
pub fn rust_cargo_fixture() -> TempDir {
    create_fixture("rust-cargo", &[
        ("Cargo.toml", r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#),
        ("src/main.rs", r#"fn main() { println!("Hello"); }"#),
    ])
}

/// Create minimal Node.js project fixture
pub fn node_npm_fixture() -> TempDir {
    create_fixture("node-npm", &[
        ("package.json", r#"{
  "name": "test-project",
  "version": "1.0.0",
  "scripts": {
    "build": "tsc",
    "start": "node dist/index.js"
  }
}"#),
        ("src/index.ts", "console.log('Hello');"),
        ("tsconfig.json", r#"{"compilerOptions": {"outDir": "dist"}}"#),
    ])
}

/// Create monorepo fixture
pub fn turborepo_fixture() -> TempDir {
    create_fixture("turborepo", &[
        ("turbo.json", r#"{"pipeline": {"build": {}}}"#),
        ("package.json", r#"{
  "name": "monorepo",
  "workspaces": ["apps/*", "packages/*"]
}"#),
        ("apps/web/package.json", r#"{"name": "web", "scripts": {"build": "next build"}}"#),
        ("apps/api/package.json", r#"{"name": "api", "scripts": {"build": "tsc"}}"#),
        ("packages/ui/package.json", r#"{"name": "ui", "scripts": {"build": "tsc"}}"#),
    ])
}

/// Assert that a UniversalBuild matches expected values
pub fn assert_detection(result: &UniversalBuild, expected: DetectionExpectation) {
    assert_eq!(result.metadata.language, expected.language, "Language mismatch");
    assert_eq!(result.metadata.build_system, expected.build_system, "Build system mismatch");
    assert!(result.metadata.confidence >= expected.min_confidence,
        "Confidence {} below minimum {}", result.metadata.confidence, expected.min_confidence);

    if let Some(expected_commands) = expected.build_commands {
        for cmd in expected_commands {
            assert!(result.build.commands.iter().any(|c| c.contains(&cmd)),
                "Expected build command '{}' not found", cmd);
        }
    }
}

pub struct DetectionExpectation {
    pub language: &'static str,
    pub build_system: &'static str,
    pub min_confidence: f32,
    pub build_commands: Option<Vec<&'static str>>,
}
