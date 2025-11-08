# CLAUDE.md - aipack Development Guide

This file provides guidance to Claude Code (claude.ai/code) when working with the aipack project.

## Development Policy

**IMPORTANT PRINCIPLES:**
1. **No Backwards Compatibility**: Breaking changes are acceptable and preferred when they improve the codebase. Never maintain compatibility with old APIs, configurations, or interfaces.
2. **No Historical Comments**: Code and documentation should reflect the current state only. Never include comments explaining what was added, removed, or changed (e.g., "removed X because...", "added Y to replace...").
3. **Clean Slate**: When refactoring, completely remove old code and update all references. The codebase should read as if it was always implemented the current way.
4. **Minimal Comments**: Keep commenting to a minimum. If code is simple and obvious it doesn't require comments. This is not a library so examples are not required.

## Project Overview

**aipack** is a Rust-based AI-powered buildkit frontend for intelligent build command detection. It uses LLM function calling with iterative tool execution to analyze repositories on-demand, avoiding context window limitations.

**Architecture**: Tool-based detection using LLM function calling
- LLM explores repositories iteratively using 6 specialized tools
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
│   ├── ai/                  # LLM integrations
│   │   ├── mod.rs           # Module definition
│   │   ├── backend.rs       # Unified LLMBackend trait
│   │   └── genai_backend.rs # GenAI multi-provider client
│   ├── detection/           # Build command detection
│   │   ├── mod.rs
│   │   ├── analyzer.rs      # Repository analyzer (legacy)
│   │   ├── prompt.rs        # System prompts
│   │   ├── response.rs      # Response parsing
│   │   ├── service.rs       # Detection orchestration
│   │   ├── types.rs         # Data structures
│   │   └── tools/           # Tool execution framework
│   │       ├── definitions.rs  # Tool name constants
│   │       ├── executor.rs  # Tool implementation (6 tools)
│   │       └── registry.rs  # JSON schemas for tools
│   ├── cli/                 # Command-line interface
│   │   ├── mod.rs
│   │   ├── commands.rs      # CLI command definitions
│   │   └── output.rs        # Output formatting
│   ├── config.rs            # Configuration management
│   └── util/                # Utilities
│       ├── mod.rs
│       ├── fs.rs            # File system utilities
│       ├── cache.rs         # Result caching
│       └── logging.rs       # Structured logging
├── tests/                   # Integration tests
│   ├── end_to_end_test.rs  # Full workflow tests
│   ├── ollama_integration.rs # Ollama backend tests
│   └── ...                  # Other integration tests
├── examples/                # Usage examples
│   └── genai_detection.rs  # Multi-provider example
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
| `submit_detection` | Submit final detection result | Return detected build system |

### Benefits

- **Scalability**: Works with large repositories without exceeding context windows
- **Efficiency**: LLM only requests files it needs
- **Accuracy**: Can explore deeply when needed
- **Flexibility**: Adapts to any project structure

## Using the GenAI Backend

The GenAI backend provides a unified interface to multiple LLM providers through the `genai` crate.

### Quick Start

```rust
use aipack::ai::backend::LLMBackend;
use aipack::ai::genai_backend::{GenAIBackend, Provider};
use std::path::PathBuf;

// Create an Ollama client (default local endpoint)
let client = GenAIBackend::new(
    Provider::Ollama,
    "qwen2.5-coder:7b".to_string(),
).await?;

// Detect build system (LLM will use tools to explore)
let result = client.detect(PathBuf::from("/path/to/repo")).await?;
println!("Build system: {}", result.build_system);
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

### Examples

#### Ollama (Local)
```rust
// Uses default localhost:11434, or set OLLAMA_HOST environment variable
let backend = GenAIBackend::new(
    Provider::Ollama,
    "qwen2.5-coder:7b".to_string(),
).await?;
```

#### Claude
```rust
// Requires ANTHROPIC_API_KEY in environment
let backend = GenAIBackend::new(
    Provider::Claude,
    "claude-sonnet-4-5-20250929".to_string(),
).await?;
```

#### OpenAI
```rust
// Requires OPENAI_API_KEY in environment
let backend = GenAIBackend::new(
    Provider::OpenAI,
    "gpt-4".to_string(),
).await?;
```

#### Custom Configuration
```rust
use std::time::Duration;

// For custom Ollama endpoint, set OLLAMA_HOST environment variable:
// std::env::set_var("OLLAMA_HOST", "http://192.168.1.100:11434");

let backend = GenAIBackend::with_config(
    Provider::Ollama,
    "qwen2.5-coder:14b".to_string(),
    Some(Duration::from_secs(120)),  // Custom timeout
    Some(1024),  // Max tokens
).await?;
```

### Running Examples

```bash
# Ollama
cargo run --example genai_detection

# With custom model
OLLAMA_MODEL=qwen2.5-coder:14b cargo run --example genai_detection

# Claude
PROVIDER=claude ANTHROPIC_API_KEY=sk-... cargo run --example genai_detection

# OpenAI
PROVIDER=openai OPENAI_API_KEY=sk-... cargo run --example genai_detection
```

## Environment Variables for GenAI Backend

The GenAI backend relies on environment variables for API authentication and configuration. The `genai` crate automatically reads these standard environment variables - **you do not need to set them programmatically in your code**.

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

### How GenAI Reads Environment Variables

The `genai` crate automatically reads these environment variables when you create a backend:

```rust
// The genai crate reads ANTHROPIC_API_KEY automatically
let backend = GenAIBackend::new(
    Provider::Claude,
    "claude-sonnet-4-5-20250929".to_string(),
).await?;
```

**You do not need to:**
- Read environment variables manually with `std::env::var()`
- Set environment variables programmatically with `std::env::set_var()`
- Pass API keys as parameters to the backend constructor

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

#### 1. Backend Trait
```rust
#[async_trait]
pub trait LLMBackend: Send + Sync {
    async fn detect(&self, context: RepositoryContext) -> Result<DetectionResult>;
    fn name(&self) -> &str;
}
```

All LLM integrations implement this trait, allowing pluggable backends.

#### 2. Repository Context
Structure containing all information about a repository:
- File tree (structure)
- Configuration files (contents)
- README (if present)
- Detected file types
- Git information (optional)

#### 3. Detection Result
Structured output containing:
- Build system identified
- Build/test/deploy commands
- Confidence score
- Reasoning/explanation
- List of detected files

### Async Design
- Use `tokio::main` for async runtime
- All I/O operations (file reading, API calls) are async
- Error handling with `?` operator and early returns

### Error Handling
- Use `anyhow::Result<T>` for application errors
- Implement `thiserror::Error` for custom error types
- Propagate errors to CLI for user-friendly messages

## Development Workflow

### Adding a New LLM Provider

To add support for a new LLM provider:

1. Add the new provider variant to the `Provider` enum in `src/ai/genai_backend.rs`
2. Update the `prefix()` and `name()` methods to handle the new provider
3. Update the `Display` implementation
4. Document the required environment variables
5. Write tests

Example:
```rust
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Ollama,
    OpenAI,
    Claude,
    Gemini,
    Grok,
    Groq,
    NewProvider,  // Add your provider here
}

impl Provider {
    fn prefix(&self) -> &'static str {
        match self {
            // ... existing cases
            Provider::NewProvider => "newprovider",
        }
    }

    fn name(&self) -> &'static str {
        match self {
            // ... existing cases
            Provider::NewProvider => "NewProvider",
        }
    }
}
```

### Modifying Prompts

Edit `src/detection/prompt.rs`:
```rust
pub fn build_detection_prompt(context: &RepositoryContext) -> String {
    format!(r#"
        Your prompt here...
        Files: {}
        Content: {}
    "#, context.file_tree, context.files_content)
}
```

Test with:
```bash
cargo test test_prompt_generation
```

### Adding Tests

```rust
#[tokio::test]
async fn test_detection() {
    let context = create_test_context();
    let client = GenAIBackend::new(
        Provider::Ollama,
        "qwen2.5-coder:7b".to_string(),
    ).await.unwrap();
    let result = client.detect(context).await.unwrap();
    assert_eq!(result.build_command, "cargo build");
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

# Tool execution configuration
AIPACK_MAX_TOOL_ITERATIONS=10      # Max conversation iterations (default: 10, max: 50)
AIPACK_TOOL_TIMEOUT=30             # Tool execution timeout in seconds (default: 30, max: 300)
AIPACK_MAX_FILE_SIZE=1048576       # Max file size to read in bytes (default: 1MB, max: 10MB)

# Logging
AIPACK_LOG_LEVEL=info              # "trace", "debug", "info", "warn", or "error"
RUST_LOG=aipack=debug,info         # Structured logging (overrides AIPACK_LOG_LEVEL)
```

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
