<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# CLAUDE.md - aipack Development Guide

This file provides guidance to Claude Code (claude.ai/code) when working with the aipack project.

## Claude Rules

The following rules are MANDATORY for CLAUDE:
 - Keep comments to the minimum, only in cases when it's required. No examples.
 - Don't keep code for backwards compatibility – remove it
 - Never postpone a task if it was asked for, never cut corners
 - Code simplicity is most important.
 - Dead code is a smell. Remove it, unless you think it will be required later – then ask the user whether it should be kept.

## Development Policy

**IMPORTANT PRINCIPLES:**
1. **No Backwards Compatibility**: Breaking changes are acceptable and preferred when they improve the codebase. Never maintain compatibility with old APIs, configurations, or interfaces.
2. **No Historical Comments**: Code and documentation should reflect the current state only. Never include comments explaining what was added, removed, or changed (e.g., "removed X because...", "added Y to replace...").
3. **Clean Slate**: When refactoring, completely remove old code and update all references. The codebase should read as if it was always implemented the current way.
4. **Minimal Comments**: Keep commenting to a minimum. If code is simple and obvious it doesn't require comments. This is not a library so examples are not required.

## Project Overview

**aipack** is a Rust-based AI-powered buildkit frontend for intelligent build command detection. It uses LLM function calling with iterative tool execution to analyze repositories on-demand, avoiding context window limitations.

**Architecture**: Tool-based detection using LLM function calling
- LLM explores repositories iteratively using 7 specialized tools
- Avoids passing full repository context upfront
- Scales to large repositories without exceeding context windows
- LLM requests only the files it needs for accurate detection

**Key Tech Stack:**
- **Language**: Rust 1.70+
- **Build System**: Cargo
- **AI Backends**: GenAI (unified multi-provider client)
  - Ollama (local inference)
  - Anthropic Claude
  - OpenAI GPT
  - Google Gemini
  - xAI Grok
  - Groq
- **HTTP Client**: reqwest (async), genai (multi-provider)
- **CLI Framework**: clap (derive macros)
- **Error Handling**: anyhow, thiserror
- **Async Runtime**: tokio
- **Serialization**: serde, serde_json

## Build & Development Commands

### Build and Compile
```bash
# Full build
cargo build

# Release build (optimized)
cargo build --release

# Check compilation (fast)
cargo check

# Clean build directory
cargo clean
```

### Running
```bash
# Development binary
cargo run -- detect

# With arguments
cargo run -- detect /path/to/repo
cargo run -- detect --format json --backend ollama

# Release binary
./target/release/aipack detect
```

### Testing
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_ollama_detection

# Run with output (print statements)
cargo test -- --nocapture

# Run integration tests only
cargo test --test '*'

# Coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

### Code Quality
```bash
# Format code (auto-fix)
cargo fmt

# Lint code
cargo clippy

# Full check before commit
cargo fmt && cargo clippy && cargo test
```

## Project Structure

```
aipack/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library root
│   ├── ai/                  # Legacy AI integration (deprecated)
│   │   ├── mod.rs
│   │   └── error.rs
│   ├── llm/                 # LLM client abstraction
│   │   ├── mod.rs           # Module definition
│   │   ├── client.rs        # LLMClient trait
│   │   ├── types.rs         # LLM request/response types
│   │   ├── genai.rs         # GenAI multi-provider client
│   │   ├── mock.rs          # MockLLMClient for testing
│   │   ├── recording.rs     # Request/response recording system
│   │   ├── selector.rs      # LLM client selection logic
│   │   ├── test_context.rs # Test utilities
│   │   └── embedded/        # Embedded local inference
│   │       ├── mod.rs
│   │       ├── client.rs    # EmbeddedClient implementation
│   │       ├── download.rs  # Model downloader
│   │       ├── hardware.rs  # Hardware detection (RAM, CUDA, Metal)
│   │       └── models.rs    # Model selection by available RAM
│   ├── languages/           # Language registry
│   │   ├── mod.rs
│   │   ├── registry.rs      # LanguageRegistry
│   │   ├── rust.rs          # Rust language definition
│   │   ├── javascript.rs    # JavaScript/TypeScript
│   │   ├── python.rs        # Python
│   │   ├── java.rs          # Java
│   │   ├── go.rs            # Go
│   │   ├── dotnet.rs        # .NET/C#
│   │   ├── ruby.rs          # Ruby
│   │   ├── php.rs           # PHP
│   │   ├── cpp.rs           # C++
│   │   └── elixir.rs        # Elixir
│   ├── fs/                  # FileSystem abstraction
│   │   ├── mod.rs
│   │   ├── trait.rs         # FileSystem trait
│   │   ├── real.rs          # RealFileSystem implementation
│   │   └── mock.rs          # MockFileSystem for testing
│   ├── bootstrap/           # Pre-scan bootstrap
│   │   ├── mod.rs
│   │   ├── scanner.rs       # BootstrapScanner
│   │   └── context.rs       # BootstrapContext, RepoSummary
│   ├── progress/            # Progress reporting
│   │   ├── mod.rs
│   │   ├── handler.rs       # ProgressHandler trait
│   │   └── logging.rs       # LoggingHandler implementation
│   ├── validation/          # Validation system
│   │   ├── mod.rs
│   │   ├── validator.rs     # Validator
│   │   └── rules.rs         # ValidationRule trait + implementations
│   ├── tools/               # Tool system
│   │   ├── mod.rs
│   │   ├── trait_def.rs     # Tool trait
│   │   ├── implementations.rs # Tool implementations (list_files, read_file, etc.)
│   │   ├── registry.rs      # ToolRegistry
│   │   ├── cache.rs         # ToolCache
│   │   └── system.rs        # ToolSystem facade
│   ├── pipeline/            # Analysis pipeline
│   │   ├── mod.rs
│   │   ├── config.rs        # PipelineConfig
│   │   ├── context.rs       # PipelineContext (owns dependencies)
│   │   └── analysis.rs      # AnalysisPipeline orchestrator
│   ├── detection/           # Detection service
│   │   ├── mod.rs
│   │   ├── service.rs       # DetectionService (public API)
│   │   ├── types.rs         # UniversalBuild and related types
│   │   └── analyzer.rs      # Legacy analyzer
│   ├── output/              # Output formatting
│   │   ├── mod.rs
│   │   ├── schema.rs        # JSON schema output
│   │   └── dockerfile.rs    # Dockerfile generation
│   ├── cli/                 # Command-line interface
│   │   ├── mod.rs
│   │   ├── commands.rs      # CLI command definitions
│   │   └── output.rs        # Output formatting
│   └── config.rs            # Configuration management
├── tests/                   # Integration tests
│   ├── e2e.rs               # End-to-end tests with fixtures
│   ├── cli_integration.rs   # CLI integration tests
│   ├── mock_detection_test.rs # Mock-based detection tests
│   ├── embedded_llm_test.rs # Embedded LLM integration tests
│   ├── bootstrap_integration_test.rs # Bootstrap scanner tests
│   ├── error_handling_test.rs # Error handling scenarios
│   ├── backend_health_test.rs # Backend health checks
│   ├── analyzer_integration.rs # Legacy analyzer tests
│   ├── fixtures/            # Test fixture repositories
│   │   ├── single-language/ # Rust, Node.js, Python, Java, Go, .NET
│   │   ├── monorepo/        # npm-workspaces, cargo-workspace, etc.
│   │   ├── edge-cases/      # empty-repo, no-manifest, nested-projects
│   │   ├── expected/        # Expected JSON outputs
│   │   └── README.md        # Fixture documentation
│   └── recordings/          # LLM request/response recordings
├── examples/                # Usage examples
│   └── genai_detection.rs   # Multi-provider example
├── Cargo.toml               # Project manifest
├── Cargo.lock               # Dependency lock
├── PRD.md                   # Product requirements
├── CHANGELOG.md             # Version history
├── README.md                # User documentation
└── CLAUDE.md                # This file
```

## Tool-Based Detection Architecture

aipack uses LLM function calling to analyze repositories iteratively instead of passing all context upfront.

### How It Works

1. **LLM receives system prompt** explaining available tools
2. **Iterative exploration**: LLM calls tools to explore the repository
3. **On-demand file reading**: Only requested files are read and sent to LLM
4. **Final submission**: LLM calls `submit_detection` with the result

### Available Tools

| Tool | Purpose | Example Use |
|------|---------|-------------|
| `list_files` | List directory contents with optional glob filtering | Find all `package.json` files |
| `read_file` | Read file contents with size limits | Read `Cargo.toml` to confirm Rust project |
| `search_files` | Search for files by name pattern | Find all `*.gradle` files |
| `get_file_tree` | Get tree view of directory structure | Understand repository layout |
| `grep_content` | Search file contents with regex | Find `"scripts"` in package.json files |
| `get_best_practices` | Get language-specific build template | Retrieve best practices for cargo/Rust |
| `submit_detection` | Submit final UniversalBuild result | Return complete build specification |

### Benefits

- **Scalability**: Works with large repositories without exceeding context windows
- **Efficiency**: LLM only requests files it needs
- **Accuracy**: Can explore deeply when needed
- **Flexibility**: Adapts to any project structure

## Using the Detection Service

The `DetectionService` is the main entry point for build system detection, orchestrating the entire analysis pipeline.

### Quick Start

```rust
use aipack::detection::DetectionService;
use aipack::llm::selector::select_llm_client;
use std::path::PathBuf;

// Select LLM client (auto-detects based on environment)
let llm_client = select_llm_client().await?;

// Create detection service
let service = DetectionService::new(llm_client)?;

// Detect build system (returns Vec<UniversalBuild>)
let results = service.detect(PathBuf::from("/path/to/repo")).await?;

for build in results {
    println!("Project: {}", build.metadata.project_name);
    println!("Build system: {}", build.metadata.build_system);
}
```

### Supported Providers

| Provider | Example Model | Environment Variable | Notes |
|----------|---------------|---------------------|-------|
| **Ollama** | `qwen2.5-coder:7b` | `OLLAMA_HOST` (optional) | Local inference, default port 11434 |
| **Claude** | `claude-sonnet-4-5-20250929` | `ANTHROPIC_API_KEY` (required) | Anthropic API |
| **OpenAI** | `gpt-4` | `OPENAI_API_KEY` (required) | OpenAI API |
| **Gemini** | `gemini-pro` | `GOOGLE_API_KEY` (required) | Google AI |
| **Grok** | `grok-1` | `XAI_API_KEY` (required) | xAI |
| **Groq** | `mixtral-8x7b-32768` | `GROQ_API_KEY` (required) | Groq |

### Provider Selection

The `select_llm_client()` function automatically selects the best available LLM client based on environment:

1. **Environment variables** - If `AIPACK_PROVIDER` is set, use that provider
2. **Ollama** - Try connecting to Ollama (localhost:11434)
3. **Embedded** - Fall back to embedded local inference (zero-config)

```rust
use aipack::llm::selector::select_llm_client;

// Auto-select based on environment
let client = select_llm_client().await?;
```

### Examples

#### Using with DetectionService
```rust
use aipack::detection::DetectionService;
use aipack::llm::selector::select_llm_client;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Auto-select LLM client
    let client = select_llm_client().await?;

    // Create service
    let service = DetectionService::new(client)?;

    // Detect
    let results = service.detect(PathBuf::from("./my-repo")).await?;

    for build in results {
        println!("{}", serde_json::to_string_pretty(&build)?);
    }

    Ok(())
}
```

#### Running with Different Providers

```bash
# Auto-select (tries Ollama, falls back to embedded)
cargo run -- detect /path/to/repo

# Force specific provider
AIPACK_PROVIDER=ollama cargo run -- detect /path/to/repo
AIPACK_PROVIDER=claude ANTHROPIC_API_KEY=sk-... cargo run -- detect /path/to/repo
AIPACK_PROVIDER=openai OPENAI_API_KEY=sk-... cargo run -- detect /path/to/repo

# Embedded (zero-config local inference)
AIPACK_PROVIDER=embedded cargo run -- detect /path/to/repo
```

## LLM Provider Environment Variables

LLM providers rely on environment variables for API authentication and configuration. The `genai` crate automatically reads these standard environment variables - **you do not need to set them programmatically in your code**.

### Required Environment Variables by Provider

#### Ollama (Local Inference)
```bash
# Optional - Custom Ollama server endpoint
# Default: http://localhost:11434
export OLLAMA_HOST=http://localhost:11434

# Or use custom port/host
export OLLAMA_HOST=http://192.168.1.100:11434
```

**Note:** Ollama doesn't require an API key. If `OLLAMA_HOST` is not set, it defaults to `http://localhost:11434`.

#### Anthropic Claude
```bash
# Required - Your Anthropic API key
export ANTHROPIC_API_KEY=sk-ant-api03-...

# Optional - Custom API endpoint (for proxies or custom deployments)
export ANTHROPIC_BASE_URL=https://api.anthropic.com
```

**Obtaining an API key:** Visit https://console.anthropic.com/settings/keys

#### OpenAI
```bash
# Required - Your OpenAI API key
export OPENAI_API_KEY=sk-proj-...

# Optional - Custom API endpoint (for Azure OpenAI or proxies)
export OPENAI_API_BASE=https://api.openai.com/v1

# Optional - Organization ID (for team accounts)
export OPENAI_ORG_ID=org-...
```

**Obtaining an API key:** Visit https://platform.openai.com/api-keys

#### Google Gemini
```bash
# Required - Your Google AI API key
export GOOGLE_API_KEY=AIza...

# Optional - Custom API endpoint
export GOOGLE_API_BASE_URL=https://generativelanguage.googleapis.com
```

**Obtaining an API key:** Visit https://makersuite.google.com/app/apikey

#### xAI Grok
```bash
# Required - Your xAI API key
export XAI_API_KEY=xai-...
```

**Obtaining an API key:** Visit https://console.x.ai/

#### Groq
```bash
# Required - Your Groq API key
export GROQ_API_KEY=gsk_...
```

**Obtaining an API key:** Visit https://console.groq.com/keys

### Setting Environment Variables

#### For a Single Command
```bash
# Ollama (no API key needed)
cargo run --example genai_detection

# Claude
ANTHROPIC_API_KEY=sk-ant-... cargo run --example genai_detection

# OpenAI
OPENAI_API_KEY=sk-proj-... cargo run --example genai_detection

# Multiple variables
PROVIDER=claude ANTHROPIC_API_KEY=sk-ant-... cargo run --example genai_detection
```

#### For Your Shell Session
```bash
# Add to ~/.bashrc, ~/.zshrc, or ~/.profile
export ANTHROPIC_API_KEY=sk-ant-api03-...
export OPENAI_API_KEY=sk-proj-...
export GOOGLE_API_KEY=AIza...

# Reload your shell
source ~/.bashrc
```

#### Using .env Files (Recommended for Development)
Create a `.env` file in your project root:
```bash
# .env
ANTHROPIC_API_KEY=sk-ant-api03-...
OPENAI_API_KEY=sk-proj-...
GOOGLE_API_KEY=AIza...
OLLAMA_HOST=http://localhost:11434
```

**Important:** Add `.env` to your `.gitignore` to avoid committing secrets!

Then use a tool like `direnv` or load manually:
```bash
# Load environment variables from .env
set -a
source .env
set +a

# Now run your command
cargo run --example genai_detection
```

### Error Messages for Missing Environment Variables

If a required environment variable is not set, you'll see helpful error messages:

```
Error: Failed to initialize Claude backend: authentication error.
Ensure ANTHROPIC_API_KEY is set in environment.
```

```
Error: Failed to initialize OpenAI backend: missing API key.
Ensure OPENAI_API_KEY is set in environment.
```

### How Environment Variables Are Used

The `genai` crate automatically reads these environment variables when initializing LLM clients through `select_llm_client()`:

```rust
use aipack::llm::selector::select_llm_client;

// The genai crate reads ANTHROPIC_API_KEY automatically when provider is Claude
let client = select_llm_client().await?;
```

**You do not need to:**
- Read environment variables manually with `std::env::var()`
- Set environment variables programmatically with `std::env::set_var()`
- Pass API keys as parameters to client constructors

**The genai crate handles all of this internally.**

### Verification

To verify your environment variables are set correctly:

```bash
# Check if variables are set (without revealing values)
env | grep -E '(ANTHROPIC|OPENAI|GOOGLE|GROQ|XAI)_API_KEY'

# Test with a simple detection
ANTHROPIC_API_KEY=sk-... cargo run -- detect /path/to/repo
```

### Security Best Practices

1. **Never commit API keys to Git:** Always use environment variables or encrypted secrets
2. **Use .env files locally:** Keep `.env` in `.gitignore`
3. **Rotate keys regularly:** Generate new API keys periodically
4. **Use separate keys per environment:** Different keys for dev/staging/production
5. **Restrict key permissions:** Use API key scopes/permissions where available

### Troubleshooting

**Problem:** "Authentication failed" or "missing API key" errors

**Solution:**
1. Verify the environment variable is set: `echo $ANTHROPIC_API_KEY`
2. Check for typos in the variable name (case-sensitive!)
3. Ensure the key is valid (not expired or revoked)
4. Try setting the variable in the same command: `ANTHROPIC_API_KEY=... cargo run`

**Problem:** "Connection refused" with Ollama

**Solution:**
1. Check if Ollama is running: `curl http://localhost:11434/api/tags`
2. Start Ollama: `ollama serve`
3. Verify the port: Ollama defaults to 11434

## Architecture & Design Patterns

### Core Concepts

#### 1. LLMClient Trait
```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
}
```

All LLM integrations implement this trait, providing pluggable backends with tool calling support.

#### 2. Pipeline Architecture
The detection pipeline consists of several phases:
- **Bootstrap** - Pre-scan repository to detect languages and manifests
- **Tool Execution** - LLM iteratively explores using available tools
- **Validation** - Validate UniversalBuild output for correctness
- **Assembly** - Return final build specification(s)

#### 3. UniversalBuild Output
Multi-stage container build specification containing:
- **Metadata**: project name, language, build system, confidence, reasoning
- **Build Stage**: base image, packages, environment variables, build commands, context, cache paths, artifacts
- **Runtime Stage**: base image, packages, environment variables, copy specifications, command, ports, healthcheck
- Schema version with validation

**Note**: For monorepos, `DetectionService.detect()` returns `Vec<UniversalBuild>` with one entry per runnable application.

### Async Design
- Use `tokio::main` for async runtime
- All I/O operations (file reading, API calls) are async
- Error handling with `?` operator and early returns

### Error Handling
- Use `anyhow::Result<T>` for application errors
- Implement `thiserror::Error` for custom error types
- Propagate errors to CLI for user-friendly messages

## Development Workflow

### Adding Tests

```rust
use aipack::detection::DetectionService;
use aipack::llm::MockLLMClient;
use std::path::PathBuf;

#[tokio::test]
async fn test_detection() {
    // Use MockLLMClient for deterministic testing
    let mock_client = MockLLMClient::new(vec![
        // Scripted responses...
    ]);

    let service = DetectionService::new(Arc::new(mock_client)).unwrap();
    let results = service.detect(PathBuf::from("tests/fixtures/rust-cargo")).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].metadata.build_system, "cargo");
    assert!(!results[0].build.commands.is_empty());
}
```

## Configuration & Environment

### Aipack Configuration Environment Variables

```bash
# Provider selection (defaults to "ollama")
AIPACK_PROVIDER=ollama             # "ollama", "openai", "claude", "gemini", "grok", or "groq"

# Model configuration
AIPACK_MODEL=qwen2.5-coder:7b      # Model name for selected provider

# Caching
AIPACK_CACHE_ENABLED=true
AIPACK_CACHE_DIR=/tmp/aipack-cache

# Request configuration
AIPACK_REQUEST_TIMEOUT=60          # Request timeout in seconds
AIPACK_MAX_CONTEXT_SIZE=512000     # Maximum context size in tokens
AIPACK_MAX_TOKENS=8192             # Max tokens per LLM response (default: 8192, min: 512, max: 128000)

# Tool execution configuration
AIPACK_MAX_TOOL_ITERATIONS=10      # Max conversation iterations (default: 10, max: 50)
AIPACK_TOOL_TIMEOUT=30             # Tool execution timeout in seconds (default: 30, max: 300)
AIPACK_MAX_FILE_SIZE=1048576       # Max file size to read in bytes (default: 1MB, max: 10MB)

# Logging
AIPACK_LOG_LEVEL=info              # "trace", "debug", "info", "warn", or "error"
RUST_LOG=aipack=debug,info         # Structured logging (overrides AIPACK_LOG_LEVEL)

# Embedded model configuration
AIPACK_MODEL_SIZE=7B               # Explicit model size: "0.5B", "1.5B", "3B", or "7B" (overrides auto-selection)
```

### Embedded Model Selection

When using the embedded backend, aipack runs local inference using Qwen2.5-Coder models in GGUF format (Q4 quantized). Model selection works as follows:

#### Automatic Selection (Default)
By default, aipack auto-selects the largest model that fits in available RAM:
```bash
# Auto-selects based on available RAM (reserves 25% or 2GB minimum for system)
./aipack detect .
```

#### Explicit Model Size Selection
Override auto-selection with `AIPACK_MODEL_SIZE` environment variable:
```bash
# Use specific model size (bypasses auto-selection)
AIPACK_MODEL_SIZE=0.5B ./aipack detect .   # Smallest (requires ~1GB RAM)
AIPACK_MODEL_SIZE=1.5B ./aipack detect .   # Small (requires ~2.5GB RAM)
AIPACK_MODEL_SIZE=3B ./aipack detect .     # Medium (requires ~4GB RAM)
AIPACK_MODEL_SIZE=7B ./aipack detect .     # Largest (requires ~5.5GB RAM)
```

**Note:** If the selected model exceeds available RAM, aipack will show a warning but still attempt to load it (may cause OOM).

#### Available Models

All models use GGUF format with Q4_K_M quantization and have embedded tokenizers:

| Model | Params | RAM Required | Quantization | Notes |
|-------|--------|--------------|--------------|-------|
| Qwen2.5-Coder 0.5B GGUF | 0.5B | 1.0GB | Q4_K_M | Smallest footprint |
| Qwen2.5-Coder 1.5B GGUF | 1.5B | 2.5GB | Q4_K_M | Faster download |
| Qwen2.5-Coder 3B GGUF | 3B | 4.0GB | Q4_K_M | Good for CI |
| Qwen2.5-Coder 7B GGUF | 7B | 5.5GB | Q4_K_M | Best quality/size ratio |

All models support tool calling and are optimized for code understanding.

### LLM Self-Reasoning Loop Prevention

aipack includes safeguards to prevent LLMs from getting stuck in self-reasoning loops:

1. **Token Limits**: `AIPACK_MAX_TOKENS` (default: 8192) prevents runaway generation
2. **Stop Sequences**: Automatically applied to catch repetitive patterns:
   - `</thinking>`
   - `In summary:`
   - `To reiterate:`
   - `Let me repeat:`
3. **Per-Call Timeouts**: Each LLM API call enforces the configured timeout
4. **Concise Prompt**: System prompt discourages verbose reasoning

### Provider-Specific Environment Variables

These are managed by the `genai` crate and should be set according to the provider documentation:

```bash
# Ollama (local inference)
OLLAMA_HOST=http://localhost:11434   # Optional, defaults to localhost:11434

# OpenAI
OPENAI_API_KEY=sk-proj-...           # Required for OpenAI
OPENAI_API_BASE=https://api.openai.com/v1  # Optional

# Anthropic Claude
ANTHROPIC_API_KEY=sk-ant-api03-...   # Required for Claude

# Google Gemini
GOOGLE_API_KEY=AIza...               # Required for Gemini

# xAI Grok
XAI_API_KEY=xai-...                  # Required for Grok

# Groq
GROQ_API_KEY=gsk_...                 # Required for Groq
```

### Configuration File

The configuration is primarily driven by environment variables. The `AipackConfig` struct has the following fields:

```rust
pub struct AipackConfig {
    pub provider: Provider,              // Ollama, OpenAI, Claude, Gemini, Grok, Groq
    pub model: String,                   // Model name for selected provider
    pub cache_enabled: bool,
    pub cache_dir: Option<PathBuf>,
    pub request_timeout_secs: u64,
    pub max_context_size: usize,
    pub log_level: String,
    pub max_tool_iterations: usize,      // Max iterations for tool-based detection
    pub tool_timeout_secs: u64,          // Timeout for individual tool executions
    pub max_file_size_bytes: usize,      // Max file size for read_file tool
}
```

All provider-specific settings (API keys, endpoints) are handled via environment variables by the `genai` crate.

## Dependencies & Versioning

### Key Dependencies
- **tokio**: Async runtime (pinned to 1.35+)
- **clap**: CLI argument parsing (4.4+)
- **reqwest**: HTTP client (0.11+ with json)
- **serde/serde_json**: JSON serialization
- **anyhow**: Error handling
- **tracing**: Structured logging

### Updating Dependencies
```bash
# Check for updates
cargo update

# Update specific package
cargo update -p tokio

# Check for security vulnerabilities
cargo audit
```

## Testing Strategy

### Unit Tests
- Test individual functions in their modules
- Location: near the function being tested (`#[cfg(test)]` modules)
- Focus: logic correctness, error cases

### Integration Tests
- Test complete workflows (end-to-end)
- Location: `tests/` directory
- Focus: backend integration, prompt generation, response parsing

### Test Fixtures
- Use `tests/fixtures/` for test repositories
- Create minimal test repos with specific build systems
- Generate dynamically when possible

Example:
```rust
#[tokio::test]
async fn test_cargo_detection() {
    let fixture = create_cargo_fixture().await;
    let client = OllamaClient::new("http://localhost:11434");
    let result = client.detect(fixture).await.unwrap();

    assert_eq!(result.build_command, "cargo build");
    assert!(result.confidence > 0.8);
}
```

## Performance Considerations

### Optimization Areas
1. **LLM Requests**: Cache results, minimize token count
2. **File I/O**: Use parallel directory walking (`walkdir`)
3. **Memory**: Stream large files, avoid loading entire repos
4. **Network**: Connection pooling in reqwest (automatic)

### Profiling
```bash
# Install flamegraph
cargo install flamegraph

# Generate profile
cargo flamegraph --bench detect

# View with
open flamegraph.svg
```

## Debugging

### Structured Logging
```rust
use tracing::{debug, info, warn, error};

debug!("Analyzing repository structure");
info!("Detected build system: cargo");
warn!("Missing key configuration file");
error!("Failed to query LLM: {}", err);
```

Enable with: `RUST_LOG=aipack=debug cargo run`

### Common Issues

**1. Ollama Connection Refused**
```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# Run Ollama
ollama serve
```

**2. Mistral API Timeout**
- Check `MISTRAL_API_KEY` is set
- Verify network connectivity
- Check API quota at mistral.ai

**3. LLM Response Parsing Failed**
- Add `--verbose` flag to see raw response
- Check prompt format matches expected JSON
- Update response parser

## Release & Versioning

### Version Numbering
Follow semantic versioning (MAJOR.MINOR.PATCH):
- **0.1.0**: Initial MVP with Ollama + Mistral
- **0.2.0**: Add confidence scoring and caching
- **1.0.0**: Stable release with platform integration

### Release Checklist
1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md` with changes
3. Run full test suite: `cargo test`
4. Run clippy: `cargo clippy`
5. Test CLI: `cargo run -- detect`
6. Commit with message: `chore: Release v0.x.x`
7. Create git tag: `git tag v0.x.x`

## Contributing Guidelines

### Code Style
- Run `cargo fmt` before committing
- Address all `cargo clippy` warnings
- Use meaningful variable names
- Keep functions under 50 lines when possible

### Commit Messages
```
feat: Add new feature
fix: Bug fix description
docs: Documentation updates
chore: Maintenance tasks
test: Add/update tests
perf: Performance improvements
```

### PR Requirements
- Tests for new functionality (>80% coverage)
- Documentation updates
- CHANGELOG entry
- Passes `cargo clippy` and `cargo fmt`

## Resources

- **Rust Book**: https://doc.rust-lang.org/book/
- **Tokio Guide**: https://tokio.rs/
- **Mistral Docs**: https://docs.mistral.ai/
- **Ollama**: https://ollama.ai/
- **Qwen Models**: https://huggingface.co/Qwen/
- **Cargo Book**: https://doc.rust-lang.org/cargo/

## IDE Setup

### VS Code
1. Install "Rust Analyzer" extension
2. Install "CodeLLDB" for debugging
3. Add to `.vscode/settings.json`:
```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

### IntelliJ IDEA
1. Install "Rust" plugin
2. Set up run configurations:
   - CLI: `cargo run -- detect`
   - Tests: `cargo test`
3. Enable Clippy on save

## Known Quirks & Important Notes

### Tokio Runtime
- `#[tokio::main]` macro automatically handles runtime initialization
- All async operations must be within async context
- Use `tokio::spawn` for concurrent tasks

### LLM Response Parsing
- Always validate JSON before parsing
- Provide helpful error messages if parsing fails
- Log raw responses in debug mode

### File System Safety
- Be careful with path handling (use `std::path::Path`)
- Consider symlink loops in directory walking
- Handle permission errors gracefully

### API Rate Limiting
- All cloud providers have rate limits; implement exponential backoff
- Consider request queuing for high volume
- Cache results aggressively to minimize API calls

## Test Fixtures

aipack includes a comprehensive test fixture library for validating build system detection across different languages, build tools, and project structures.

### Fixture Directory Structure

```
tests/fixtures/
├── single-language/   # Single build system projects
│   ├── rust-cargo/        # Standard Rust project with Cargo.toml
│   ├── rust-workspace/    # Cargo workspace with multiple members
│   ├── node-npm/          # TypeScript + npm + Express + Jest
│   ├── node-yarn/         # Same as node-npm but with yarn.lock
│   ├── node-pnpm/         # Same as node-npm but with pnpm-lock.yaml
│   ├── python-pip/        # Flask app with requirements.txt + pytest
│   ├── python-poetry/     # Flask app using Poetry (pyproject.toml)
│   ├── java-maven/        # Spring Boot app with Maven (pom.xml)
│   ├── java-gradle/       # Spring Boot app with Gradle (build.gradle)
│   ├── kotlin-gradle/     # Spring Boot app in Kotlin with Gradle Kotlin DSL
│   ├── go-mod/            # Gin web server with go.mod
│   └── dotnet-csproj/     # ASP.NET Core minimal API with .csproj
├── monorepo/          # Monorepo/workspace projects
│   ├── npm-workspaces/    # npm workspaces with packages/* and apps/*
│   ├── turborepo/         # Turborepo configuration with turbo.json
│   ├── cargo-workspace/   # Rust workspace (same as rust-workspace)
│   ├── gradle-multiproject/ # Gradle multi-project with settings.gradle
│   ├── maven-multimodule/ # Maven multi-module with parent/child poms
│   └── polyglot/          # Mixed: Frontend (Node.js), Backend (Java), CLI (Rust)
├── edge-cases/        # Edge cases and unusual configurations
│   ├── empty-repo/        # Completely empty repository (only README)
│   ├── no-manifest/       # Source code without build manifest
│   ├── multiple-manifests/ # Mixed build systems (Cargo + npm + Maven)
│   ├── nested-projects/   # Projects within projects (outer/inner)
│   └── vendor-heavy/      # Project with large vendor directory
├── expected/          # Expected JSON outputs for validation
│   ├── rust-cargo.json
│   ├── node-npm.json
│   ├── npm-workspaces.json
│   └── ...
└── README.md          # Fixture documentation
```

### Creating Test Fixtures

All fixtures follow these principles:

1. **Minimal**: Only essential files for detection
2. **Representative**: Real-world project structures
3. **Working**: Can actually build/run with the specified tools
4. **Complete**: Include source code, manifests, and dependencies

Example minimal Rust fixture:

```rust
// tests/fixtures/single-language/rust-cargo/Cargo.toml
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = "1.0"

// tests/fixtures/single-language/rust-cargo/src/main.rs
fn main() {
    println!("Hello, world!");
}
```

### Expected Outputs

Each fixture has a corresponding `expected/{fixture-name}.json` file containing the expected `UniversalBuild` output. These serve as golden files for regression testing.

Example:

```json
{
  "version": "1.0",
  "metadata": {
    "project_name": "test-project",
    "language": "Rust",
    "build_system": "cargo",
    "confidence": "high"
  },
  "build": {
    "base_image": "rust:1.75",
    "commands": ["cargo build --release"],
    "artifacts": ["target/release/test-project"]
  },
  "runtime": {
    "base_image": "debian:bookworm-slim",
    "command": "./test-project"
  }
}
```

### Using Fixtures in Tests

```rust
use std::path::PathBuf;

#[tokio::test]
async fn test_rust_cargo_detection() {
    let fixture_path = PathBuf::from("tests/fixtures/single-language/rust-cargo");
    let expected_path = PathBuf::from("tests/fixtures/expected/rust-cargo.json");

    // Run detection
    let result = detect(fixture_path).await.unwrap();

    // Load expected output
    let expected: UniversalBuild = load_expected(&expected_path).unwrap();

    // Validate
    assert_eq!(result.metadata.language, expected.metadata.language);
    assert_eq!(result.metadata.build_system, expected.metadata.build_system);
    assert_eq!(result.build.commands, expected.build.commands);
}
```

## LLM Recording System

The recording system captures LLM request/response pairs for deterministic testing without requiring live LLM access. This enables:

- **CI/CD testing** without API keys or Ollama setup
- **Regression testing** against known-good LLM responses
- **Reproducible tests** that don't depend on LLM behavior changes
- **Faster tests** by replaying cached responses

### Recording Modes

Controlled via the `AIPACK_RECORDING_MODE` environment variable:

```bash
# Record mode: Make live LLM calls and save responses
AIPACK_RECORDING_MODE=record cargo test

# Replay mode: Use saved responses, fail if recording missing
AIPACK_RECORDING_MODE=replay cargo test

# Auto mode (default): Replay if recording exists, otherwise record
AIPACK_RECORDING_MODE=auto cargo test
```

### Recording Directory

Recordings are stored in `tests/recordings/` with filenames based on request content hash:

```
tests/recordings/
├── cli_integration_test_detect_json_format__0251b6ea.json
├── e2e_test_rust_cargo__3f4a2b1c.json
└── ...
```

### Recording Format

Each recording file contains the complete LLM conversation:

```json
{
  "request_hash": "0251b6ea7cc524a746fec47169ee1d43",
  "request": {
    "messages": [
      {
        "role": "system",
        "content": "You are an expert build system analyzer..."
      },
      {
        "role": "user",
        "content": "Analyze the repository..."
      },
      {
        "role": "assistant",
        "content": "",
        "tool_calls": [
          {
            "call_id": "embedded_0",
            "name": "read_file",
            "arguments": {
              "path": "Cargo.toml"
            }
          }
        ]
      },
      {
        "role": "tool",
        "content": "{...}",
        "tool_call_id": "embedded_0"
      }
    ],
    "temperature": 0.1,
    "max_tokens": 8192
  },
  "response": {
    "content": "",
    "tool_calls": [
      {
        "call_id": "submit_0",
        "name": "submit_detection",
        "arguments": {
          "version": "1.0",
          "metadata": {...},
          "build": {...},
          "runtime": {...}
        }
      }
    ]
  }
}
```

### Using Recordings in Tests

The recording system is transparent when using `LLMClient` trait:

```rust
use aipack::llm::client::LLMClient;
use aipack::llm::recording::{RecordingLLMClient, RecordingMode};

#[tokio::test]
async fn test_with_recording() {
    // Wrap any LLM client with RecordingLLMClient
    let base_client = create_llm_client().await;
    let client = RecordingLLMClient::new(
        base_client,
        RecordingMode::Auto,
        PathBuf::from("tests/recordings"),
    );

    // Use normally - recording happens automatically
    let result = client.chat(messages).await.unwrap();
}
```

### Recording Best Practices

1. **Commit recordings to Git**: Enables deterministic CI/CD testing
2. **Use `auto` mode locally**: Records missing, replays existing
3. **Use `replay` mode in CI**: Ensures tests don't make live API calls
4. **Re-record periodically**: Update when prompts or LLM behavior changes
5. **Review recordings**: Ensure recorded responses are correct before committing

### Updating Recordings

To update recordings after prompt changes:

```bash
# Delete old recordings
rm -rf tests/recordings/

# Re-record with live LLM
AIPACK_RECORDING_MODE=record cargo test

# Verify new recordings work
AIPACK_RECORDING_MODE=replay cargo test

# Commit updated recordings
git add tests/recordings/
git commit -m "chore: Update LLM recordings after prompt changes"
```

### Recording Configuration

Additional environment variables:

```bash
# Custom recordings directory (default: tests/recordings)
AIPACK_RECORDINGS_DIR=/path/to/recordings

# Recording mode (default: auto)
AIPACK_RECORDING_MODE=auto  # "record", "replay", or "auto"
```
