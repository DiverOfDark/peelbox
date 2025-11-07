# Aipack Configuration Guide

This guide covers all configuration options for aipack.

## Quick Start

```rust
use aipack::{AipackConfig, init_default};

// Initialize logging
init_default();

// Load configuration
let config = AipackConfig::default();

// Validate
config.validate()?;

// Get backend
let backend = config.selected_backend_config()?;
```

## Environment Variables

### Backend Selection

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `AIPACK_BACKEND` | `auto`, `ollama`, `mistral` | `auto` | Which LLM backend to use |

**Auto mode behavior:**
1. Try Ollama first (checks if endpoint is reachable)
2. Fall back to Mistral if API key is set
3. Error if neither is available

### Ollama Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `AIPACK_OLLAMA_ENDPOINT` | `http://localhost:11434` | Ollama service URL |
| `AIPACK_OLLAMA_MODEL` | `qwen2.5-coder:7b` | Model name to use |

**Example:**
```bash
export AIPACK_BACKEND=ollama
export AIPACK_OLLAMA_ENDPOINT=http://localhost:11434
export AIPACK_OLLAMA_MODEL=qwen:14b
```

### Mistral Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `MISTRAL_API_KEY` | None (required) | Mistral API key |
| `AIPACK_MISTRAL_MODEL` | `mistral-small` | Model name to use |

**Example:**
```bash
export AIPACK_BACKEND=mistral
export MISTRAL_API_KEY=your-api-key-here
export AIPACK_MISTRAL_MODEL=mistral-medium
```

### Caching

| Variable | Default | Description |
|----------|---------|-------------|
| `AIPACK_CACHE_ENABLED` | `true` | Enable result caching |
| `AIPACK_CACHE_DIR` | `/tmp/aipack-cache` | Cache directory path |

**Example:**
```bash
export AIPACK_CACHE_ENABLED=true
export AIPACK_CACHE_DIR=/home/user/.cache/aipack
```

### Runtime Parameters

| Variable | Default | Min | Max | Description |
|----------|---------|-----|-----|-------------|
| `AIPACK_REQUEST_TIMEOUT` | `30` | 1 | 600 | Timeout in seconds |
| `AIPACK_MAX_CONTEXT_SIZE` | `512000` | 1024 | 10485760 | Max context bytes |

**Example:**
```bash
export AIPACK_REQUEST_TIMEOUT=60
export AIPACK_MAX_CONTEXT_SIZE=1048576  # 1MB
```

### Logging

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `AIPACK_LOG_LEVEL` | `trace`, `debug`, `info`, `warn`, `error` | `info` | Log level |
| `AIPACK_LOG_JSON` | `true`, `false` | `false` | Use JSON output |
| `RUST_LOG` | Filter syntax | (see below) | Standard Rust log filtering |

**Examples:**
```bash
# Debug level
export AIPACK_LOG_LEVEL=debug

# JSON output
export AIPACK_LOG_JSON=true

# Fine-grained filtering
export RUST_LOG=aipack=debug,reqwest=warn
```

## Configuration in Code

### Loading Configuration

```rust
use aipack::AipackConfig;

// From environment with defaults
let config = AipackConfig::default();

// Manual construction
let config = AipackConfig {
    backend: "ollama".to_string(),
    ollama_endpoint: "http://localhost:11434".to_string(),
    ollama_model: "qwen2.5-coder:7b".to_string(),
    mistral_api_key: None,
    mistral_model: "mistral-small".to_string(),
    cache_enabled: true,
    cache_dir: Some(PathBuf::from("/tmp/cache")),
    request_timeout_secs: 30,
    max_context_size: 512_000,
    log_level: "info".to_string(),
};
```

### Validation

```rust
// Validate configuration
match config.validate() {
    Ok(()) => println!("Configuration valid"),
    Err(e) => eprintln!("Configuration error: {}", e),
}
```

Validation checks:
- Backend is valid (auto/ollama/mistral)
- Endpoint URLs are properly formatted
- Required API keys are present
- Timeout is 1-600 seconds
- Context size is 1KB-10MB
- Log level is valid

### Backend Selection

```rust
use aipack::BackendConfig;

// Get configured backend
let backend = config.selected_backend_config()?;

match backend {
    BackendConfig::Local { endpoint, model, .. } => {
        println!("Using Ollama at {} with {}", endpoint, model);
    }
    BackendConfig::OpenAI { model, .. } => {
        println!("Using Mistral with {}", model);
    }
    _ => {}
}
```

### Checking Availability

```rust
// Check if Ollama is reachable
if config.is_ollama_available() {
    println!("Ollama is available");
}

// Check if Mistral API key is set
if config.has_mistral_key() {
    println!("Mistral API key is configured");
}
```

### Cache Paths

```rust
// Get cache path for a repository
if config.cache_enabled {
    let path = config.cache_path("my-repo");
    println!("Cache path: {}", path.display());
    // Output: /tmp/aipack-cache/my-repo.json
}

// Special characters are sanitized
let path = config.cache_path("user/repo:branch");
// Output: /tmp/aipack-cache/user_repo_branch.json
```

## Logging Setup

### Basic Initialization

```rust
use aipack;

// Default configuration (INFO level, console output)
aipack::init_default();

// From environment variables
aipack::init_from_env();

// Specific log level
aipack::with_level("debug");
```

### Advanced Configuration

```rust
use aipack::{LoggingConfig, init_logging};
use tracing::Level;

// Custom configuration
let config = LoggingConfig {
    level: Level::DEBUG,
    use_json: false,
    include_target: true,
    include_location: true,
    include_thread_ids: false,
};
init_logging(config);

// Or use presets
let config = LoggingConfig::development();  // DEBUG, console
let config = LoggingConfig::production();   // INFO, JSON
init_logging(config);
```

### Using Structured Logging

```rust
use tracing::{info, debug, warn, error};

// Basic logging
info!("Application started");

// With structured data
debug!(repo = "myrepo", files = 42, "Analyzing repository");

// With error context
warn!(error = ?err, endpoint = "http://localhost:11434", "Connection failed");

// Error logging
error!(cause = %err, "Critical failure");
```

## Error Handling

### Configuration Errors

```rust
use aipack::ConfigError;

match config.validate() {
    Err(ConfigError::InvalidBackend(name)) => {
        eprintln!("Invalid backend: {}", name);
        eprintln!("Valid options: auto, ollama, mistral");
    }
    Err(ConfigError::MissingApiKey) => {
        eprintln!("Set MISTRAL_API_KEY environment variable");
    }
    Err(ConfigError::InvalidEndpoint(url)) => {
        eprintln!("Invalid endpoint URL: {}", url);
    }
    Err(ConfigError::ValidationFailed(msg)) => {
        eprintln!("Validation failed: {}", msg);
    }
    Ok(()) => {}
}
```

## Common Scenarios

### Development Setup

```bash
# Use local Ollama with debug logging
export AIPACK_BACKEND=ollama
export AIPACK_OLLAMA_MODEL=qwen2.5-coder:7b
export AIPACK_LOG_LEVEL=debug
export AIPACK_CACHE_ENABLED=true
```

### Production Setup

```bash
# Use Mistral API with production settings
export AIPACK_BACKEND=mistral
export MISTRAL_API_KEY=your-production-key
export AIPACK_MISTRAL_MODEL=mistral-medium
export AIPACK_LOG_LEVEL=warn
export AIPACK_LOG_JSON=true
export AIPACK_REQUEST_TIMEOUT=60
export AIPACK_CACHE_ENABLED=true
export AIPACK_CACHE_DIR=/var/cache/aipack
```

### Testing Setup

```bash
# Disable caching, verbose logging
export AIPACK_BACKEND=ollama
export AIPACK_LOG_LEVEL=trace
export AIPACK_CACHE_ENABLED=false
export RUST_LOG=aipack=trace
```

### CI/CD Setup

```bash
# Use API with structured logging
export AIPACK_BACKEND=mistral
export MISTRAL_API_KEY=${CI_MISTRAL_KEY}
export AIPACK_LOG_JSON=true
export AIPACK_LOG_LEVEL=info
export AIPACK_CACHE_ENABLED=false
```

## Troubleshooting

### "No backend available" error

**Symptom:**
```
Auto mode: No backend available. Ollama is not reachable and Mistral API key is not set
```

**Solutions:**
1. Start Ollama: `ollama serve`
2. Set Mistral API key: `export MISTRAL_API_KEY=your-key`
3. Explicitly select backend: `export AIPACK_BACKEND=ollama`

### "Ollama is not reachable" error

**Check:**
```bash
# Test Ollama endpoint
curl http://localhost:11434/api/tags

# Start Ollama if not running
ollama serve
```

### "Mistral API key not set" error

**Solution:**
```bash
export MISTRAL_API_KEY=your-api-key-here
```

Get your API key from: https://console.mistral.ai/

### Logging not appearing

**Check:**
1. Logging is initialized: `aipack::init_default()`
2. Log level is appropriate: `export AIPACK_LOG_LEVEL=debug`
3. Filter allows your module: `export RUST_LOG=aipack=debug`

### Configuration validation fails

**Debug:**
```rust
use aipack::AipackConfig;

let config = AipackConfig::default();
println!("{}", config);  // Show all values

match config.validate() {
    Ok(()) => println!("Valid"),
    Err(e) => eprintln!("Error: {}", e),  // Shows specific issue
}
```

## Best Practices

### In Development
- Use `AIPACK_BACKEND=auto` to automatically select available backend
- Enable debug logging: `AIPACK_LOG_LEVEL=debug`
- Enable caching to speed up repeated operations

### In Production
- Explicitly set `AIPACK_BACKEND` (don't rely on auto)
- Use `AIPACK_LOG_JSON=true` for structured logs
- Set appropriate timeout: `AIPACK_REQUEST_TIMEOUT=60`
- Configure persistent cache: `AIPACK_CACHE_DIR=/var/cache/aipack`

### Security
- Never commit `MISTRAL_API_KEY` to version control
- Use environment variable management (e.g., vault, secrets manager)
- Rotate API keys regularly
- In logs, API keys are automatically redacted ("Set" vs "Not set")

## See Also

- [Examples](../examples/) - Working code examples
- [API Documentation](https://docs.rs/aipack) - Full API reference
- [Logging Guide](LOGGING_GUIDE.md) - Detailed logging documentation
