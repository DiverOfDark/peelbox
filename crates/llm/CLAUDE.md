# peelbox-llm

Unified LLM abstraction layer with multiple backend implementations: cloud APIs (OpenAI, Anthropic, Gemini, Groq, xAI), local Ollama, and embedded Candle-based inference.

## Module Structure

```
src/
├── lib.rs              # Public API re-exports
├── types.rs            # ChatMessage, LLMRequest, LLMResponse, ToolCall, ToolDefinition
├── client.rs           # LLMClient trait (async chat interface)
├── genai.rs            # GenAIClient (cloud provider adapter via genai crate)
├── embedded/
│   ├── client.rs       # EmbeddedClient (local Candle inference)
│   ├── hardware.rs     # HardwareDetector (GPU/CPU/RAM detection)
│   ├── models.rs       # ModelSelector (Qwen 7B/3B/1.5B selection by RAM)
│   └── download.rs     # ModelDownloader (HuggingFace Hub)
├── selector.rs         # select_llm_client() -- provider fallback chain
├── lazy.rs             # LazyLLMClient (deferred initialization)
├── recording.rs        # RecordingLLMClient (request/response recording for tests)
├── mock.rs             # MockLLMClient (queue-based test double)
└── test_context.rs     # TestContext (test name discovery for recordings)
```

## Core Trait

```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat(&self, request: LLMRequest) -> Result<LLMResponse, BackendError>;
    fn name(&self) -> &str;
    fn model_info(&self) -> Option<String> { None }
}
```

All implementations conform to this trait. Only the **first tool call** from a response is captured (`LLMResponse::tool_call` is `Option<ToolCall>`, not `Vec`).

## Provider Selection

`select_llm_client()` tries providers in this order:
1. **Configured provider** (if credentials available, skip Ollama)
2. **Ollama** (probes `OLLAMA_HOST` or `localhost:11434` with 2s timeout)
3. **Embedded** (auto-selects model by available RAM, reserves 25%)

## Embedded Inference

- Models: Qwen2.5-Coder GGUF (7B/5.5GB, 3B/4GB, 1.5B/2.5GB)
- Deterministic: seed=42, temperature=0.0 (greedy)
- Device priority: CUDA > Metal > CPU (with fallback)
- Minimum 3GB available RAM required
- `PEELBOX_MODEL_SIZE` env var overrides auto-selection ("1.5B", "3B", "7B")

## Recording System

`RecordingLLMClient` wraps any client for deterministic test replay:

| Mode | Behavior |
|------|----------|
| `Record` | Always calls backend, saves to JSON |
| `Replay` | Only uses cached recordings |
| `Auto` | Records on cache miss, replays on hit |

Path normalization for determinism: cwd -> `[REPO_ROOT]`, `/tmp` -> `[TEMP_DIR]`, UUIDs -> `[UUID]`. Request hash (MD5) used as filename.

## Feature Flags

```toml
[features]
cuda = ["candle-core/cuda", ...]   # NVIDIA GPU support
metal = ["candle-core/metal", ...]  # Apple Silicon support
```

Cloud providers work without any features. Embedded always has CPU fallback.

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `PEELBOX_PROVIDER` | Provider selection |
| `PEELBOX_MODEL` | Model name |
| `PEELBOX_API_BASE_URL` | Custom API endpoint |
| `PEELBOX_MODEL_SIZE` | Force embedded model size |
| `PEELBOX_RECORDING_MODE` | record, replay, auto |
| `PEELBOX_RECORDINGS_DIR` | Recording directory (default tests/recordings) |
| `PEELBOX_TEST_NAME` | Override test name for recordings |
| `OLLAMA_HOST` | Ollama endpoint |
| `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, etc. | Provider credentials |

## Important Notes

- **Single tool call only** -- subsequent tool calls in a response are dropped
- **Embedded determinism** -- fixed seed + greedy sampling = reproducible across hosts
- **Recording normalization is critical for CI** -- without it, recordings fail on different machines
- **Interactive mode** checks `stdin.is_terminal()` before prompting for model download
- **CUDA detection** uses unsafe C API (`cuInit`) -- compile with `cuda` feature to enable
- **Tool call JSON parsing (embedded)** expects strict `{"name": "...", "arguments": {...}}`

## Tests

~40 tests. Run with: `cargo test -p peelbox-llm`

Key test patterns:
- `#[tokio::test]` for async LLMClient implementations
- `MockLLMClient` with FIFO queue for response injection
- `tempfile::TempDir` for isolated recording directories
