# aipack Development Guide

This guide helps contributors set up their development environment, understand the codebase, and contribute effectively to aipack.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Setting Up Development Environment](#setting-up-development-environment)
- [Building from Source](#building-from-source)
- [Running Tests](#running-tests)
- [Code Style and Conventions](#code-style-and-conventions)
- [Adding New Features](#adding-new-features)
- [Adding New LLM Backends](#adding-new-llm-backends)
- [Testing New Features](#testing-new-features)
- [Debugging](#debugging)
- [Common Development Tasks](#common-development-tasks)
- [Release Process](#release-process)

## Prerequisites

### Required Tools

- **Rust 1.70+**: Install from [rustup.rs](https://rustup.rs/)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **Cargo**: Comes with Rust installation

- **Git**: For version control
  ```bash
  # Ubuntu/Debian
  sudo apt install git

  # macOS
  brew install git
  ```

### Optional Tools

- **Ollama**: For local testing with Ollama backend
  ```bash
  # macOS
  brew install ollama

  # Linux
  curl -fsSL https://ollama.ai/install.sh | sh
  ```

- **Just**: Command runner for common tasks
  ```bash
  cargo install just
  ```

- **Cargo Watch**: Auto-rebuild on file changes
  ```bash
  cargo install cargo-watch
  ```

- **Cargo Tarpaulin**: Code coverage
  ```bash
  cargo install cargo-tarpaulin
  ```

### Recommended IDE Setup

#### VS Code

1. Install extensions:
   - Rust Analyzer
   - CodeLLDB (for debugging)
   - Better TOML
   - Error Lens

2. Configure settings (`.vscode/settings.json`):
   ```json
   {
     "rust-analyzer.checkOnSave.command": "clippy",
     "rust-analyzer.cargo.features": "all",
     "editor.formatOnSave": true,
     "[rust]": {
       "editor.defaultFormatter": "rust-lang.rust-analyzer"
     }
   }
   ```

3. Configure launch for debugging (`.vscode/launch.json`):
   ```json
   {
     "version": "0.2.0",
     "configurations": [
       {
         "type": "lldb",
         "request": "launch",
         "name": "Debug aipack",
         "cargo": {
           "args": ["build", "--bin=aipack"],
           "filter": {
             "name": "aipack",
             "kind": "bin"
           }
         },
         "args": ["detect", "/path/to/test/repo"],
         "cwd": "${workspaceFolder}"
       }
     ]
   }
   ```

#### IntelliJ IDEA / CLion

1. Install Rust plugin
2. Import project as Cargo project
3. Set up run configurations:
   - **Run**: `cargo run -- detect`
   - **Test**: `cargo test`
   - **Clippy**: `cargo clippy --all-targets`

## Setting Up Development Environment

### 1. Clone Repository

```bash
git clone https://github.com/diverofdark/aipack.git
cd 
```

### 2. Install Dependencies

```bash
# Install Rust dependencies (automatic on first build)
cargo fetch

# Install development tools
cargo install cargo-watch cargo-tarpaulin
```

### 3. Configure Environment

Create a `.env` file for local development:

```bash
# .env (add to .gitignore)
AIPACK_BACKEND=ollama
AIPACK_OLLAMA_ENDPOINT=http://localhost:11434
AIPACK_OLLAMA_MODEL=qwen:7b
RUST_LOG=aipack=debug,info
```

Load environment variables:
```bash
# Bash/Zsh
source .env

# Or use direnv
echo "source .env" > .envrc
direnv allow
```

### 4. Set Up Ollama (for testing)

```bash
# Start Ollama server
ollama serve

# In another terminal, pull model
ollama pull qwen:7b

# Verify it works
curl http://localhost:11434/api/tags
```

### 5. Verify Setup

```bash
# Build project
cargo build

# Run tests
cargo test

# Run aipack
cargo run -- detect .

# Check code quality
cargo clippy
cargo fmt --check
```

## Building from Source

### Development Build

```bash
# Fast incremental build
cargo build

# Binary location
./target/debug/aipack detect /path/to/repo
```

### Release Build

```bash
# Optimized build (slower compilation, faster runtime)
cargo build --release

# Binary location
./target/release/aipack detect /path/to/repo
```

### Build Options

```bash
# Check compilation without building
cargo check

# Build with all features
cargo build --all-features

# Build specific binary
cargo build --bin aipack

# Clean build artifacts
cargo clean

# Build and run
cargo run -- detect /path/to/repo
```

### Cross-Compilation

```bash
# Install target
rustup target add x86_64-unknown-linux-musl

# Build for target
cargo build --release --target x86_64-unknown-linux-musl
```

## Running Tests

### All Tests

```bash
# Run all tests (unit + integration + doc tests)
cargo test

# Run with output visible
cargo test -- --nocapture

# Run tests in parallel
cargo test -- --test-threads=4
```

### Unit Tests

```bash
# Run tests in a specific module
cargo test detection::analyzer

# Run a specific test
cargo test test_ollama_client

# Run tests matching pattern
cargo test ollama
```

### Integration Tests

```bash
# Run all integration tests
cargo test --test '*'

# Run specific integration test file
cargo test --test ollama_integration

# Run with Ollama backend (requires Ollama running)
AIPACK_BACKEND=ollama cargo test
```

### Documentation Tests

```bash
# Run doc tests
cargo test --doc

# Test specific module docs
cargo test --doc detection::service
```

### Coverage

```bash
# Generate coverage report
cargo tarpaulin --out Html

# Open coverage report
open tarpaulin-report.html

# Coverage with all tests
cargo tarpaulin --all-features --workspace
```

### Performance Testing

```bash
# Run with timing information
cargo test -- --nocapture --show-output

# Benchmark (requires nightly)
cargo +nightly bench
```

## Code Style and Conventions

### Formatting

```bash
# Format all code
cargo fmt

# Check formatting without changing files
cargo fmt --check

# Format specific file
rustfmt src/lib.rs
```

### Linting

```bash
# Run Clippy
cargo clippy

# Clippy for all targets
cargo clippy --all-targets

# Clippy with strict settings
cargo clippy -- -D warnings

# Fix auto-fixable issues
cargo clippy --fix
```

### Naming Conventions

- **Types**: `PascalCase` (e.g., `DetectionResult`, `LLMBackend`)
- **Functions**: `snake_case` (e.g., `detect`, `build_prompt`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_TIMEOUT`)
- **Modules**: `snake_case` (e.g., `detection`, `ai`)

### Documentation Standards

Every public item must have documentation:

```rust
/// Detects build system for a repository
///
/// This function analyzes the repository structure and uses an LLM
/// backend to determine the appropriate build system and commands.
///
/// # Arguments
///
/// * `repo_path` - Path to the repository root
///
/// # Returns
///
/// A `DetectionResult` containing build system information
///
/// # Errors
///
/// Returns `ServiceError` if:
/// - Repository path does not exist
/// - Analysis fails
/// - LLM backend is unavailable
///
/// # Example
///
/// ```no_run
/// use aipack::detect;
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let result = detect(PathBuf::from("/path/to/repo")).await?;
/// println!("Build command: {}", result.build_command);
/// # Ok(())
/// # }
/// ```
pub async fn detect(repo_path: PathBuf) -> Result<DetectionResult, ServiceError> {
    // Implementation
}
```

### Error Handling

Use `Result` types and avoid `unwrap()` in library code:

```rust
// Good
fn parse_config(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::IoError(e))?;
    serde_json::from_str(&content)
        .map_err(|e| ConfigError::ParseError(e.to_string()))
}

// Bad (in library code)
fn parse_config(path: &Path) -> Config {
    let content = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&content).unwrap()
}
```

### Async Conventions

- Use `async/await` for all I/O operations
- Avoid blocking calls in async contexts
- Use `tokio::spawn` for concurrent tasks

```rust
// Good
async fn read_files(paths: Vec<PathBuf>) -> Result<Vec<String>, Error> {
    let mut tasks = vec![];
    for path in paths {
        tasks.push(tokio::spawn(async move {
            tokio::fs::read_to_string(path).await
        }));
    }

    let results = futures::future::join_all(tasks).await;
    // Process results
}

// Bad (sequential)
async fn read_files(paths: Vec<PathBuf>) -> Result<Vec<String>, Error> {
    let mut contents = vec![];
    for path in paths {
        contents.push(tokio::fs::read_to_string(path).await?);
    }
    Ok(contents)
}
```

## Adding New Features

### Feature Development Workflow

1. **Create Issue/Discussion**
   - Describe the feature
   - Discuss design approach
   - Get feedback from maintainers

2. **Create Feature Branch**
   ```bash
   git checkout -b feature/my-new-feature
   ```

3. **Implement Feature**
   - Write code following conventions
   - Add comprehensive tests
   - Update documentation

4. **Test Thoroughly**
   ```bash
   cargo test
   cargo clippy
   cargo fmt
   ```

5. **Submit Pull Request**
   - Clear description of changes
   - Link to related issues
   - Ensure CI passes

### Example: Adding a New Command

1. **Define command structure** (`src/cli/commands.rs`):
   ```rust
   #[derive(Debug, Args)]
   pub struct AnalyzeArgs {
       /// Repository path to analyze
       #[arg(default_value = ".")]
       pub path: PathBuf,

       /// Show detailed analysis
       #[arg(long)]
       pub detailed: bool,
   }
   ```

2. **Add to command enum**:
   ```rust
   #[derive(Debug, Subcommand)]
   pub enum Commands {
       Detect(DetectArgs),
       Analyze(AnalyzeArgs),  // New command
       // ...
   }
   ```

3. **Implement handler** (`src/cli/commands.rs`):
   ```rust
   pub async fn handle_analyze(args: AnalyzeArgs) -> Result<()> {
       // Implementation
   }
   ```

4. **Update main dispatcher** (`src/main.rs`):
   ```rust
   match cli.command {
       Commands::Detect(args) => handle_detect(args).await,
       Commands::Analyze(args) => handle_analyze(args).await,
       // ...
   }
   ```

5. **Add tests**:
   ```rust
   #[tokio::test]
   async fn test_analyze_command() {
       let args = AnalyzeArgs {
           path: PathBuf::from("test/repo"),
           detailed: true,
       };
       let result = handle_analyze(args).await;
       assert!(result.is_ok());
   }
   ```

6. **Update documentation**:
   - Add to README.md
   - Update CHANGELOG.md
   - Add example to docs/EXAMPLES.md

## Adding New LLM Backends

### Step-by-Step Guide

#### 1. Create Backend Module

Create `src/ai/claude.rs`:

```rust
use crate::ai::backend::{BackendError, LLMBackend};
use crate::detection::types::{DetectionResult, RepositoryContext};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub struct ClaudeClient {
    api_key: String,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

impl ClaudeClient {
    pub fn new(api_key: String, model: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        Self {
            api_key,
            model,
            client,
        }
    }

    pub async fn health_check(&self) -> Result<bool, BackendError> {
        // Implementation
        Ok(true)
    }
}

#[async_trait]
impl LLMBackend for ClaudeClient {
    async fn detect(&self, context: RepositoryContext) -> Result<DetectionResult, BackendError> {
        // Build prompt
        let prompt = crate::detection::prompt::build_detection_prompt(&context);

        // Create request
        let request = ClaudeRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: 2048,
        };

        // Send request
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .map_err(|e| BackendError::NetworkError {
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(BackendError::ApiError {
                message: format!("HTTP {}", response.status()),
                status_code: Some(response.status().as_u16()),
            });
        }

        let claude_response: ClaudeResponse = response
            .json()
            .await
            .map_err(|e| BackendError::InvalidResponse {
                message: e.to_string(),
                raw_response: None,
            })?;

        // Parse detection result
        let text = &claude_response.content[0].text;
        crate::detection::response::parse_detection_response(text)
    }

    fn name(&self) -> &str {
        "Claude"
    }

    fn model_info(&self) -> Option<String> {
        Some(self.model.clone())
    }
}
```

#### 2. Add to Backend Config

Update `src/ai/backend.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BackendConfig {
    Claude {
        api_key: String,
        model: String,
        api_endpoint: Option<String>,
        timeout_seconds: Option<u64>,
        max_tokens: Option<u32>,
    },
    // ... existing variants
}
```

#### 3. Update Service Factory

Update `src/detection/service.rs`:

```rust
use crate::ai::claude::ClaudeClient;

async fn create_backend(config: BackendConfig) -> Result<Arc<dyn LLMBackend>, ServiceError> {
    match config {
        BackendConfig::Claude { api_key, model, timeout_seconds, .. } => {
            let client = ClaudeClient::new(api_key, model);

            match client.health_check().await {
                Ok(true) => Ok(Arc::new(client) as Arc<dyn LLMBackend>),
                _ => Err(ServiceError::BackendInitError(
                    "Claude backend not available".to_string()
                )),
            }
        }
        // ... existing variants
    }
}
```

#### 4. Add Module to `ai/mod.rs`

```rust
pub mod backend;
pub mod claude;  // New
pub mod ollama;
```

#### 5. Update Configuration

Update `src/config.rs` to support Claude:

```rust
pub fn selected_backend_config(&self) -> Result<BackendConfig, ConfigError> {
    match self.backend.as_str() {
        "claude" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| ConfigError::MissingApiKey("ANTHROPIC_API_KEY".to_string()))?;
            let model = std::env::var("AIPACK_CLAUDE_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-5-20250929".to_string());

            Ok(BackendConfig::Claude {
                api_key,
                model,
                api_endpoint: None,
                timeout_seconds: Some(30),
                max_tokens: Some(2048),
            })
        }
        // ... existing cases
    }
}
```

#### 6. Write Tests

Create `tests/claude_integration.rs`:

```rust
#[cfg(test)]
mod claude_tests {
    use aipack::ai::claude::ClaudeClient;
    use aipack::ai::backend::LLMBackend;
    use aipack::detection::types::RepositoryContext;
    use std::path::PathBuf;

    #[tokio::test]
    #[ignore]  // Requires API key
    async fn test_claude_detection() {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY not set");

        let client = ClaudeClient::new(
            api_key,
            "claude-sonnet-4-5-20250929".to_string(),
        );

        let context = RepositoryContext::minimal(
            PathBuf::from("/test/rust/project"),
            "project/\n├── Cargo.toml\n└── src/".to_string(),
        ).with_key_file(
            "Cargo.toml".to_string(),
            "[package]\nname = \"test\"\n".to_string(),
        );

        let result = client.detect(context).await.unwrap();
        assert_eq!(result.build_system, "cargo");
    }

    #[tokio::test]
    #[ignore]
    async fn test_claude_health_check() {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY not set");

        let client = ClaudeClient::new(
            api_key,
            "claude-sonnet-4-5-20250929".to_string(),
        );

        let health = client.health_check().await.unwrap();
        assert!(health);
    }
}
```

#### 7. Update Documentation

- Add to README.md usage examples
- Update CONFIGURATION_GUIDE.md with Claude settings
- Add to EXAMPLES.md
- Update CHANGELOG.md

## Testing New Features

### Test Checklist

- [ ] Unit tests for new functions
- [ ] Integration tests for workflows
- [ ] Error case testing
- [ ] Documentation tests
- [ ] Manual testing with real data
- [ ] Performance testing (if relevant)

### Creating Test Fixtures

```bash
# Create test repository structure
mkdir -p tests/fixtures/rust-project/src
echo '[package]\nname = "test"' > tests/fixtures/rust-project/Cargo.toml
echo 'fn main() {}' > tests/fixtures/rust-project/src/main.rs
```

Use in tests:
```rust
#[tokio::test]
async fn test_with_fixture() {
    let fixture_path = PathBuf::from("tests/fixtures/rust-project");
    // Use fixture
}
```

## Debugging

### Logging

Enable debug logging:
```bash
RUST_LOG=aipack=debug cargo run -- detect /path/to/repo
```

Add logging to code:
```rust
use tracing::{debug, info, warn, error};

debug!("Analyzing repository: {:?}", path);
info!("Detection completed successfully");
warn!("Using fallback configuration");
error!("Failed to connect to backend: {}", err);
```

### Debugger

#### VS Code + CodeLLDB

1. Set breakpoint in code
2. Press F5 to start debugging
3. Use debug console to inspect variables

#### Command Line (rust-gdb)

```bash
# Build with debug symbols
cargo build

# Run with debugger
rust-gdb ./target/debug/aipack
(gdb) run detect /path/to/repo
(gdb) backtrace
```

### Common Issues

**Issue**: Tests fail with "Ollama not available"
```bash
# Solution: Start Ollama
ollama serve
```

**Issue**: Compilation errors after pulling latest
```bash
# Solution: Clean and rebuild
cargo clean
cargo build
```

**Issue**: Clippy warnings
```bash
# Solution: Fix warnings
cargo clippy --fix --allow-dirty
```

## Common Development Tasks

### Update Dependencies

```bash
# Check for outdated dependencies
cargo outdated

# Update to latest compatible versions
cargo update

# Update specific dependency
cargo update -p tokio
```

### Generate Documentation

```bash
# Build and open docs
cargo doc --no-deps --open

# Build docs for all dependencies
cargo doc --open
```

### Run Specific Example

```bash
cargo run --example basic_detect
cargo run --example custom_config
```

### Profile Performance

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --bin aipack -- detect /large/repo

# View flamegraph.svg
```

## Release Process

### Version Bump

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Commit changes
4. Create git tag

```bash
# Update version
vim Cargo.toml  # Change version = "0.1.0" to "0.2.0"

# Update changelog
vim CHANGELOG.md

# Commit
git add Cargo.toml CHANGELOG.md
git commit -m "chore: Release v0.2.0"

# Tag
git tag v0.2.0
git push origin v0.2.0
```

### Build Release Artifacts

```bash
# Build release binary
cargo build --release

# Run tests one more time
cargo test --release

# Verify binary works
./target/release/aipack --version
./target/release/aipack detect /path/to/repo
```

### Publish to crates.io

```bash
# Verify package
cargo package --list

# Dry run
cargo publish --dry-run

# Publish
cargo login
cargo publish
```

## Getting Help

- **Documentation**: Read docs/ directory
- **Issues**: Check GitHub issues
- **Discussions**: GitHub Discussions
- **Chat**: Project Discord/Slack (if available)

## Contributing Guidelines

See [CONTRIBUTING.md](../CONTRIBUTING.md) for:
- Code of conduct
- Pull request process
- Review guidelines
- Community standards
