# aipack Architecture

This document describes the high-level architecture of aipack, an AI-powered buildkit frontend for intelligent build command detection.

## Table of Contents

- [System Overview](#system-overview)
- [Architecture Diagram](#architecture-diagram)
- [Core Components](#core-components)
- [Data Flow](#data-flow)
- [Error Handling Strategy](#error-handling-strategy)
- [Extension Points](#extension-points)
- [Design Patterns](#design-patterns)
- [Performance Considerations](#performance-considerations)

## System Overview

aipack is designed as a modular, AI-powered build detection system that analyzes repository structure and uses Large Language Models (LLMs) to intelligently determine appropriate build commands. The architecture follows a clean separation of concerns with distinct layers for CLI, business logic, and AI integration.

### Key Design Principles

1. **AI-First Detection**: No hardcoded rules; all detection is LLM-driven
2. **Backend Agnostic**: Support for multiple LLM providers through a common trait
3. **Async Throughout**: All I/O operations are asynchronous for maximum efficiency
4. **Type Safety**: Leverages Rust's type system to prevent invalid states
5. **Extensibility**: Plugin-like architecture for adding new backends
6. **Testability**: Clear boundaries between components enable comprehensive testing

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Layer (main.rs)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ detect cmd   │  │ health cmd   │  │ config cmd   │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
└─────────┼──────────────────┼──────────────────┼─────────────────┘
          │                  │                  │
          └──────────────────┼──────────────────┘
                             │
┌────────────────────────────┼─────────────────────────────────────┐
│                    Service Layer                                 │
│                             │                                     │
│  ┌─────────────────────────▼────────────────────────┐           │
│  │         DetectionService                          │           │
│  │                                                    │           │
│  │  - Orchestrates detection workflow                │           │
│  │  - Validates repository paths                     │           │
│  │  - Manages backend lifecycle                      │           │
│  │  - Enriches results with metadata                 │           │
│  └─────────────┬───────────────────┬─────────────────┘           │
└────────────────┼───────────────────┼─────────────────────────────┘
                 │                   │
        ┌────────▼────────┐   ┌─────▼─────────────┐
        │                 │   │                    │
┌───────┴───────────┐     │   │  ┌─────────────────▼──────────────┐
│  Analysis Layer   │     │   │  │       AI Backend Layer          │
│                   │     │   │  │                                 │
│ ┌───────────────┐ │     │   │  │  ┌──────────────────────────┐ │
│ │ Repository    │ │     │   │  │  │    LLMBackend Trait      │ │
│ │ Analyzer      │ │     │   │  │  │                          │ │
│ │               │ │     │   │  │  │  - detect()              │ │
│ │ - Scan files  │ │     │   │  │  │  - name()                │ │
│ │ - Build tree  │ │     │   │  │  │  - model_info()          │ │
│ │ - Read configs│ │     │   │  │  └────────┬─────────────────┘ │
│ │ - Gather Git  │ │     │   │  │           │                   │
│ │   metadata    │ │     │   │  │  ┌────────▼──────────────┐   │
│ └───────┬───────┘ │     │   │  │  │                       │   │
│         │         │     │   │  │  │  Implementations:     │   │
│         ▼         │     │   │  │  │                       │   │
│ ┌───────────────┐ │     │   │  │  │  ┌─────────────────┐ │   │
│ │ Repository    │ │     │   │  │  │  │ OllamaClient    │ │   │
│ │ Context       │ │     │   │  │  │  │                 │ │   │
│ │               │◄┼─────┼───┼──┼──┼──┤ - Local models  │ │   │
│ │ - repo_path   │ │     │   │  │  │  │ - HTTP client   │ │   │
│ │ - file_tree   │ │     │   │  │  │  │ - Health check  │ │   │
│ │ - key_files   │ │     │   │  │  │  └─────────────────┘ │   │
│ │ - git_info    │ │     │   │  │  │                       │   │
│ └───────────────┘ │     │   │  │  │  ┌─────────────────┐ │   │
│                   │     │   │  │  │  │ Future:         │ │   │
└───────────────────┘     │   │  │  │  │ - ClaudeClient  │ │   │
                          │   │  │  │  │ - OpenAIClient  │ │   │
                          │   │  │  │  │ - MistralClient │ │   │
                          │   │  │  │  └─────────────────┘ │   │
                          │   │  └──────────────────────────────┘
                          │   │
                          │   │
┌─────────────────────────┼───┼────────────────────────────────────┐
│     Supporting Modules  │   │                                    │
│                         │   │                                    │
│  ┌──────────────┐       │   │    ┌─────────────────┐            │
│  │ Config       │◄──────┘   │    │ Prompt Builder  │            │
│  │              │            │    │                 │            │
│  │ - Env vars   │            │    │ - Constructs    │            │
│  │ - Defaults   │            │    │   LLM prompts   │            │
│  │ - Validation │            │    │ - Injects       │            │
│  └──────────────┘            │    │   context       │            │
│                              │    └─────────────────┘            │
│  ┌──────────────┐            │                                   │
│  │ Logging      │            │    ┌─────────────────┐            │
│  │              │            │    │ Response Parser │            │
│  │ - Structured │            │    │                 │            │
│  │ - Tracing    │            │    │ - JSON parsing  │            │
│  │ - Levels     │            └───►│ - Validation    │            │
│  └──────────────┘                 │ - Sanitization  │            │
│                                    └─────────────────┘            │
│  ┌──────────────┐                                                │
│  │ Output       │                 ┌─────────────────┐            │
│  │              │                 │ Detection Result│            │
│  │ - JSON       │                 │                 │            │
│  │ - YAML       │                 │ - build_system  │            │
│  │ - Human      │◄────────────────┤ - commands      │            │
│  └──────────────┘                 │ - confidence    │            │
│                                    │ - metadata      │            │
└───────────────────────────────────│ - warnings      │────────────┘
                                     └─────────────────┘
```

## Core Components

### 1. CLI Layer (`src/cli/`)

**Responsibility**: User interface and command-line argument processing

**Key Files**:
- `commands.rs`: Command definitions and argument parsing
- `output.rs`: Output formatting (JSON, YAML, human-readable)

**Design**:
- Uses `clap` derive macros for declarative CLI definition
- Minimal logic; delegates to service layer
- Handles output formatting based on user preference

### 2. Service Layer (`src/detection/service.rs`)

**Responsibility**: Orchestration and business logic

**Key Responsibilities**:
- Initialize and manage LLM backends
- Coordinate repository analysis and detection
- Validate inputs and sanitize outputs
- Provide helpful error messages
- Track performance metrics

**Design**:
- Single entry point (`DetectionService`)
- Thread-safe (can be shared via `Arc`)
- Backend-agnostic (works with any `LLMBackend` implementation)

### 3. Analysis Layer (`src/detection/analyzer.rs`)

**Responsibility**: Repository scanning and context building

**Key Responsibilities**:
- Walk directory tree
- Identify key configuration files
- Build file tree representation
- Extract Git metadata (if available)
- Limit data size to prevent LLM context overflow

**Design**:
- Async file I/O for performance
- Configurable limits and exclusions
- Respects `.gitignore` patterns
- Builds structured `RepositoryContext`

### 4. AI Backend Layer (`src/ai/`)

**Responsibility**: LLM integration and inference

**Key Files**:
- `backend.rs`: Core `LLMBackend` trait and types
- `ollama.rs`: Ollama client implementation
- Future: `claude.rs`, `openai.rs`, `mistral.rs`

**Design**:
- Trait-based abstraction for multiple backends
- Each backend handles its own API communication
- Health checks for availability verification
- Timeout and retry logic
- Response parsing and validation

### 5. Supporting Modules

**Configuration (`src/config.rs`)**:
- Environment variable loading
- Default value management
- Backend selection logic
- Validation and error reporting

**Logging (`src/util/logging.rs`)**:
- Structured logging with `tracing`
- Configurable log levels
- JSON output option for automation

**Prompt Building (`src/detection/prompt.rs`)**:
- Constructs LLM prompts from repository context
- Provides clear instructions and examples
- Optimizes token usage

**Response Parsing (`src/detection/response.rs`)**:
- Parses LLM JSON responses
- Validates required fields
- Handles malformed responses gracefully

## Data Flow

### Detection Workflow

```
1. User invokes CLI
   └─> aipack detect /path/to/repo

2. CLI parses arguments
   └─> Creates AipackConfig from environment and args

3. DetectionService initialization
   ├─> Validates configuration
   ├─> Selects appropriate backend
   ├─> Creates LLMBackend instance
   └─> Performs health check

4. Repository analysis
   ├─> Validates repository path exists
   ├─> Scans directory structure
   ├─> Builds file tree representation
   ├─> Reads key configuration files
   ├─> Extracts Git metadata (if available)
   └─> Creates RepositoryContext

5. Prompt construction
   ├─> Builds detection prompt from context
   ├─> Includes file tree
   ├─> Includes key file contents
   └─> Adds instructions and examples

6. LLM inference
   ├─> Sends prompt to backend API
   ├─> Waits for response (with timeout)
   └─> Receives JSON response

7. Response processing
   ├─> Parses JSON
   ├─> Validates structure
   ├─> Extracts detection fields
   ├─> Calculates confidence if missing
   └─> Creates DetectionResult

8. Result enrichment
   ├─> Adds processing time
   ├─> Adds backend information
   ├─> Generates warnings if needed
   └─> Validates commands

9. Output formatting
   ├─> Formats according to --format flag
   ├─> JSON, YAML, or human-readable
   └─> Prints to stdout

10. Error handling (if any step fails)
    ├─> Catch and classify error
    ├─> Generate helpful error message
    ├─> Provide troubleshooting hints
    └─> Exit with appropriate code
```

### Context Structure

```rust
RepositoryContext {
    repo_path: PathBuf,           // "/path/to/repo"
    file_tree: String,            // "repo/\n├── Cargo.toml\n├── src/..."
    key_files: HashMap<String, String>,  // {"Cargo.toml": "...", "README.md": "..."}
    git_info: Option<GitInfo>,    // Git metadata if available
}

GitInfo {
    current_branch: Option<String>,
    remote_url: Option<String>,
    has_uncommitted: bool,
}
```

### Result Structure

```rust
DetectionResult {
    build_system: String,          // "cargo"
    language: String,              // "Rust"
    build_command: String,         // "cargo build --release"
    test_command: String,          // "cargo test"
    deploy_command: String,        // "cargo build --release"
    confidence: f64,               // 0.0 - 1.0
    reasoning: Option<String>,     // Explanation from LLM
    detected_files: Vec<String>,   // Key files used for detection
    warnings: Vec<String>,         // Potential issues
    processing_time_ms: u64,       // Total time in milliseconds
}
```

## Error Handling Strategy

aipack employs a layered error handling approach:

### 1. Domain-Specific Errors

Each layer defines its own error types using `thiserror`:

- `BackendError`: LLM API errors (timeout, auth, parsing)
- `AnalysisError`: Repository scanning errors (permission, size)
- `ServiceError`: High-level orchestration errors
- `ConfigError`: Configuration validation errors

### 2. Error Conversion

Errors automatically convert up the stack:
```rust
AnalysisError -> ServiceError -> CLI error message
BackendError -> ServiceError -> CLI error message
```

### 3. User-Friendly Messages

The `ServiceError::help_message()` method provides:
- Clear error description
- Contextual troubleshooting steps
- Configuration hints
- Links to documentation

### 4. Error Recovery

- Graceful degradation when optional features fail
- Automatic backend fallback (Ollama -> Mistral -> error)
- Retry logic for transient failures
- Detailed logging for debugging

### 5. Exit Codes

```
0  - Success
1  - General error
2  - Configuration error
3  - Repository not found
4  - Backend unavailable
5  - Detection failed
```

## Extension Points

### Adding New LLM Backends

The architecture makes it easy to add new LLM providers:

1. **Create backend implementation**:
   ```rust
   // src/ai/claude.rs
   pub struct ClaudeClient {
       api_key: String,
       model: String,
       client: reqwest::Client,
   }

   #[async_trait]
   impl LLMBackend for ClaudeClient {
       async fn detect(&self, context: RepositoryContext) -> Result<DetectionResult, BackendError> {
           // Implementation
       }

       fn name(&self) -> &str {
           "Claude"
       }
   }
   ```

2. **Add to backend configuration**:
   ```rust
   // src/ai/backend.rs
   pub enum BackendConfig {
       Claude {
           api_key: String,
           model: String,
       },
       // ...
   }
   ```

3. **Update service factory**:
   ```rust
   // src/detection/service.rs
   async fn create_backend(config: BackendConfig) -> Result<Arc<dyn LLMBackend>, ServiceError> {
       match config {
           BackendConfig::Claude { api_key, model } => {
               Ok(Arc::new(ClaudeClient::new(api_key, model)))
           }
           // ...
       }
   }
   ```

### Adding New Output Formats

To add a new output format (e.g., XML, Markdown):

1. **Add to OutputFormat enum**:
   ```rust
   // src/cli/output.rs
   pub enum OutputFormat {
       Json,
       Yaml,
       Human,
       Markdown,  // New format
   }
   ```

2. **Implement formatter**:
   ```rust
   impl DetectionResult {
       pub fn format_markdown(&self) -> String {
           // Implementation
       }
   }
   ```

3. **Update CLI**:
   ```rust
   // src/cli/commands.rs
   #[arg(long, value_enum)]
   format: OutputFormat,
   ```

### Adding Analysis Features

To enhance repository analysis:

1. **Extend RepositoryContext**:
   ```rust
   pub struct RepositoryContext {
       // Existing fields...
       dependencies: Option<HashMap<String, String>>,  // New field
   }
   ```

2. **Update analyzer**:
   ```rust
   // src/detection/analyzer.rs
   async fn extract_dependencies(&self) -> HashMap<String, String> {
       // Implementation
   }
   ```

3. **Update prompt**:
   ```rust
   // src/detection/prompt.rs
   pub fn build_detection_prompt(context: &RepositoryContext) -> String {
       // Include dependencies in prompt
   }
   ```

## Design Patterns

### 1. Trait-Based Abstraction

The `LLMBackend` trait enables polymorphism for different LLM providers:
```rust
async fn detect_with_any_backend(
    backend: Arc<dyn LLMBackend>,
    context: RepositoryContext,
) -> Result<DetectionResult, BackendError> {
    backend.detect(context).await
}
```

### 2. Builder Pattern

`RepositoryContext` uses builder methods:
```rust
let context = RepositoryContext::minimal(path, tree)
    .with_key_file("Cargo.toml", content)
    .with_git_info(git_info);
```

### 3. Newtype Pattern

Wraps primitive types for type safety:
```rust
pub struct Confidence(f64);  // Ensures 0.0 <= x <= 1.0
```

### 4. Error Propagation

Extensive use of `?` operator for clean error handling:
```rust
async fn detect(&self, path: PathBuf) -> Result<DetectionResult, ServiceError> {
    let context = self.analyzer.analyze().await?;
    let result = self.backend.detect(context).await?;
    Ok(result)
}
```

### 5. Dependency Injection

Services receive dependencies via constructors:
```rust
pub struct DetectionService {
    backend: Arc<dyn LLMBackend>,
}

impl DetectionService {
    pub fn new(backend: Arc<dyn LLMBackend>) -> Self {
        Self { backend }
    }
}
```

## Performance Considerations

### 1. Async I/O

All file operations and network requests are async:
- Non-blocking directory traversal
- Concurrent file reads
- Parallel HTTP requests

### 2. Memory Efficiency

- Stream large files instead of loading fully
- Limit context size to prevent OOM
- Use `Arc` for shared data

### 3. Network Optimization

- Connection pooling (via `reqwest`)
- Request timeouts
- Retry with exponential backoff

### 4. Caching Opportunities

Future enhancements can add caching:
- Repository context cache (by path + mtime)
- LLM response cache (by context hash)
- Model download cache (for local backends)

### 5. Profiling

Key metrics tracked:
- Repository analysis time
- LLM inference time
- Total processing time
- Memory usage
- API call count

## Security Considerations

### 1. Path Traversal

- Validate all repository paths
- Canonicalize paths before use
- Prevent access outside repository root

### 2. File Content Safety

- Limit file size to prevent DoS
- Sanitize file contents before LLM
- Avoid including secrets in prompts

### 3. API Key Handling

- Never log API keys
- Load from environment only
- Support secure key storage

### 4. Dependency Security

- Regular `cargo audit` runs
- Pinned dependency versions
- Security-focused dependency selection

## Testing Strategy

### Unit Tests

- Each module has `#[cfg(test)]` section
- Test individual functions in isolation
- Mock external dependencies

### Integration Tests

- `tests/` directory for end-to-end tests
- Test full workflows with real backends
- Use test fixtures for consistency

### Documentation Tests

- Examples in doc comments are tested
- Ensures documentation stays up-to-date
- Provides usage examples

### Performance Tests

- Benchmark critical paths
- Track performance regression
- Optimize based on data

## Future Architecture Evolution

### Phase 2 Enhancements

- **Caching Layer**: Add persistent cache for repeated queries
- **Plugin System**: Dynamic backend loading
- **Streaming**: Stream LLM responses for faster feedback
- **Parallel Analysis**: Analyze multiple repos concurrently

### Phase 3 - Web Service & Advanced Features

- **REST/HTTP API**: Expose detection service as web API
- **Database**: Persistent storage for detection history (optional)
- **Queue System**: Async job processing for large repos
- **Monitoring**: Metrics and observability
- **Batch Processing**: Analyze multiple repositories efficiently

## References

- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [Error Handling in Rust](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [API Design Patterns](https://rust-lang.github.io/api-guidelines/)
- [Ollama Documentation](https://ollama.ai/docs)
