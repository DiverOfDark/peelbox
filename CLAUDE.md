# CLAUDE.md - aipack Development Guide

This file provides guidance to Claude Code (claude.ai/code) when working with the aipack project.

## Project Overview

**aipack** is a Rust-based AI-powered buildkit frontend for intelligent build command detection. It uses LLM analysis (Mistral API, or local LLMs via OpenAI-compatible APIs like Ollama/LM Studio) to detect repository build systems without hardcoded heuristics.

**Key Tech Stack:**
- **Language**: Rust 1.70+
- **Build System**: Cargo
- **AI Backends**:
  - OpenAI-compatible (Ollama, LM Studio, any compatible service)
  - Mistral API (cloud)
- **HTTP Client**: reqwest (async)
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
│   │   ├── backend.rs       # Backend trait and enums
│   │   ├── openai_compatible.rs  # Unified OpenAI-compatible client (Ollama, LM Studio, etc.)
│   │   └── mistral.rs       # Mistral API client (TODO)
│   ├── detection/           # Build command detection
│   │   ├── mod.rs
│   │   ├── analyzer.rs      # Repository analyzer
│   │   ├── prompt.rs        # Prompt construction
│   │   └── response.rs      # Response parsing
│   ├── cli/                 # Command-line interface
│   │   ├── mod.rs
│   │   └── commands.rs      # CLI command definitions
│   └── util/                # Utilities
│       ├── mod.rs
│       ├── fs.rs            # File system utilities
│       ├── cache.rs         # Result caching
│       └── logging.rs       # Structured logging
├── tests/                   # Integration tests
│   ├── end_to_end.rs       # Full workflow tests
│   └── fixtures/            # Test repositories
├── examples/                # Usage examples
│   └── basic_detection.rs
├── Cargo.toml               # Project manifest
├── Cargo.lock               # Dependency lock
├── PRD.md                   # Product requirements
├── CHANGELOG.md             # Version history
├── README.md                # User documentation
└── CLAUDE.md                # This file
```

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

### Adding a New LLM Backend

1. Create new file in `src/ai/` (e.g., `openai.rs`)
2. Implement `LLMBackend` trait
3. Add to backend selection logic in `src/ai/mod.rs`
4. Write tests in `tests/`
5. Update documentation

Example:
```rust
// src/ai/newprovider.rs
pub struct NewProviderClient {
    api_key: String,
    endpoint: String,
}

#[async_trait]
impl LLMBackend for NewProviderClient {
    async fn detect(&self, context: RepositoryContext) -> Result<DetectionResult> {
        // Implementation
    }

    fn name(&self) -> &str {
        "newprovider"
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
async fn test_ollama_detection() {
    let context = create_test_context();
    let client = OllamaClient::new("http://localhost:11434");
    let result = client.detect(context).await.unwrap();
    assert_eq!(result.build_command, "cargo build");
}
```

## Configuration & Environment

### Environment Variables

```bash
# Backend selection
AIPACK_BACKEND=auto                # "ollama", "lm-studio", "mistral", or "auto" (default)
AIPACK_OLLAMA_ENDPOINT=http://localhost:11434
AIPACK_OLLAMA_MODEL=qwen2.5-coder:7b

# LM Studio configuration (OpenAI-compatible local inference)
AIPACK_LM_STUDIO_ENDPOINT=http://localhost:8000

# Mistral configuration (cloud API)
MISTRAL_API_KEY=your-api-key
AIPACK_MISTRAL_MODEL=mistral-small

# Caching
AIPACK_CACHE_ENABLED=true
AIPACK_CACHE_DIR=/tmp/aipack-cache

# Logging
RUST_LOG=aipack=debug,info         # Structured logging
```

### Configuration File
Can add support for `aipack.toml`:
```toml
[ai]
backend = "auto"                    # or "ollama", "lm-studio", "mistral"

[ollama]
endpoint = "http://localhost:11434"
model = "qwen2.5-coder:7b"

[lm_studio]
endpoint = "http://localhost:8000"

[mistral]
api_key = "${MISTRAL_API_KEY}"
model = "mistral-small"

[cache]
enabled = true
ttl_seconds = 86400
```

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
- Mistral has rate limits; implement exponential backoff
- Consider request queuing for high volume
- Cache results aggressively to minimize API calls
