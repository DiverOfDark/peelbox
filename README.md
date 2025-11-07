# aipack

AI-powered buildkit frontend for intelligent build command detection.

Automatically detects build systems and generates correct build commands using LLM analysis (Mistral API, local Qwen via Ollama, or LM Studio).

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Usage](#usage)
- [Configuration](#configuration)
- [Output Formats](#output-formats)
- [Examples](#examples)
- [Supported Languages](#supported-languages)
- [Documentation](#documentation)
- [Development](#development)
- [Performance](#performance)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [Roadmap](#roadmap)
- [FAQ](#faq)

## Overview

**aipack** eliminates friction when building unfamiliar repositories by leveraging AI to automatically determine:
- What build system is used (cargo, npm, maven, gradle, etc.)
- What commands build, test, and deploy the code
- Project-specific build variations and configurations

No hardcoded detection rules. AI-driven analysis of repository structure and configuration.

### Why aipack?

- **AI-First**: Uses LLM reasoning instead of brittle pattern matching
- **Language Agnostic**: Works with any programming language or build system
- **Fast & Local**: Option to run entirely offline with Ollama
- **Flexible Integration**: Standalone CLI or library for embedding in other tools
- **Transparent**: Provides confidence scores and reasoning for all detections

## Features

- **AI-Powered Detection**: Uses LLM analysis instead of hardcoded pattern matching
- **Multiple Backends**: Support for Mistral API and local Ollama (Qwen models)
- **Fast Local Inference**: Run completely offline with local Ollama installation
- **Language Agnostic**: Detect build commands for any project type (Rust, Node.js, Java, Python, Go, and more)
- **Flexible Output**: JSON, YAML, and human-readable formats
- **Confidence Scoring**: Know how reliable each detection is
- **Detailed Reasoning**: Understand why aipack chose specific commands
- **Configurable**: Extensive environment variable configuration
- **Well-Tested**: Comprehensive test suite with integration tests
- **Production-Ready**: Error handling, logging, and performance optimization

## Quick Start

Get started in under 5 minutes:

### 1. Install Ollama (Recommended for local use)

```bash
# macOS
brew install ollama

# Linux
curl -fsSL https://ollama.ai/install.sh | sh

# Start Ollama
ollama serve &

# Pull a model
ollama pull qwen2.5-coder:7b
```

### 2. Install aipack

```bash
# From crates.io (when published)
cargo install aipack

# Or build from source
git clone https://github.com/diverofdark/aipack.git
cd 
cargo build --release
sudo cp target/release/aipack /usr/local/bin/
```

### 3. Detect your first repository

```bash
cd /path/to/your/project
aipack detect
```

Example output:
```
Build System: cargo
Language: Rust
Build Command: cargo build --release
Test Command: cargo test
Deploy Command: cargo build --release
Confidence: 98%

Detected Files:
  - Cargo.toml
  - Cargo.lock
  - src/main.rs

Reasoning: Repository contains Cargo.toml with standard Rust project structure.
Binary crate with tests configured.

Processing Time: 2.3s
```

That's it! aipack automatically detected the build system and provided the correct commands.

## Installation

### Prerequisites

- **Rust 1.70+**: Install from [rustup.rs](https://rustup.rs/)
- **Ollama** (optional, for local backend): Install from [ollama.ai](https://ollama.ai/)
- **LM Studio** (optional, for local backend): Install from [lmstudio.ai](https://lmstudio.ai/)
- **Mistral API Key** (optional, for cloud backend): Get from [console.mistral.ai](https://console.mistral.ai/)

### From Crates.io

```bash
cargo install aipack
```

### From Source

```bash
git clone https://github.com/diverofdark/aipack.git
cd 
cargo build --release
sudo install -m 755 target/release/aipack /usr/local/bin/
```

### Verify Installation

```bash
aipack --version
```

## Usage

### Basic Detection

Detect build system for current directory:

```bash
aipack detect
```

Detect specific repository:

```bash
aipack detect /path/to/repository
```

### Using Ollama (Local)

```bash
# Ensure Ollama is running
ollama serve &

# Pull model if needed
ollama pull qwen2.5-coder:7b

# Detect
aipack detect
```

### Using LM Studio (Local)

```bash
# Start LM Studio application
# (Launch the LM Studio desktop app or run server)

# Verify LM Studio is running at default port
curl http://localhost:8000/v1/models

# Detect (uses LM Studio automatically if available)
aipack detect

# Or explicitly use LM Studio backend
aipack detect --backend lm-studio
```

### Using Mistral API (Cloud)

```bash
# Set API key
export MISTRAL_API_KEY=your-api-key

# Use Mistral backend
aipack detect --backend mistral
```

### Auto Backend Selection

aipack automatically chooses the best available backend in this order:
1. Ollama (fastest for local models)
2. LM Studio (alternative local backend)
3. Mistral API (cloud fallback if configured)

```bash
# Automatically selects the best available backend
aipack detect
```

## Configuration

### Environment Variables

Configure aipack using environment variables:

```bash
# Backend Selection
export AIPACK_BACKEND=auto                # "ollama", "lm-studio", "mistral", or "auto" (default)

# Ollama Configuration
export AIPACK_OLLAMA_ENDPOINT=http://localhost:11434
export AIPACK_OLLAMA_MODEL=qwen2.5-coder:7b        # or qwen:14b, qwen:32b

# LM Studio Configuration
export AIPACK_LM_STUDIO_ENDPOINT=http://localhost:8000

# Mistral Configuration
export MISTRAL_API_KEY=your-api-key
export AIPACK_MISTRAL_MODEL=mistral-small # or mistral-medium, mistral-large

# Logging
export RUST_LOG=aipack=info               # debug, info, warn, error
```

### Configuration File

Create `.env` file in your project:

```bash
# .env
AIPACK_BACKEND=ollama
AIPACK_OLLAMA_MODEL=qwen2.5-coder:7b
RUST_LOG=aipack=info
```

Load it:

```bash
source .env
aipack detect
```

For detailed configuration options, see [docs/CONFIGURATION_GUIDE.md](docs/CONFIGURATION_GUIDE.md).

## Output Formats

### Human-Readable (Default)

```bash
aipack detect
```

```
Build System: cargo
Language: Rust
Build Command: cargo build --release
...
```

### JSON

```bash
aipack detect --format json
```

```json
{
  "buildSystem": "cargo",
  "language": "Rust",
  "buildCommand": "cargo build --release",
  "testCommand": "cargo test",
  "deployCommand": "cargo build --release",
  "confidence": 0.98,
  "reasoning": "Repository contains Cargo.toml...",
  "detectedFiles": ["Cargo.toml", "src/main.rs"],
  "warnings": [],
  "processingTimeMs": 2340
}
```

### YAML

```bash
aipack detect --format yaml
```

```yaml
buildSystem: cargo
language: Rust
buildCommand: cargo build --release
confidence: 0.98
...
```

### Parsing with jq

```bash
# Extract just the build command
aipack detect --format json | jq -r '.buildCommand'

# Get confidence as percentage
aipack detect --format json | jq '.confidence * 100'

# Check if confidence is high
if [ $(aipack detect --format json | jq '.confidence') > 0.9 ]; then
    echo "High confidence detection"
fi
```

## Examples

### Basic Usage

```bash
# Detect and build automatically
BUILD_CMD=$(aipack detect --format json | jq -r '.buildCommand')
eval "$BUILD_CMD"
```

### Scripting

```bash
#!/bin/bash
# auto-build.sh - Universal build script

DETECTION=$(aipack detect --format json)
CONFIDENCE=$(echo "$DETECTION" | jq '.confidence')

if (( $(echo "$CONFIDENCE < 0.8" | bc -l) )); then
    echo "Low confidence, manual review needed"
    exit 1
fi

BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')
TEST_CMD=$(echo "$DETECTION" | jq -r '.testCommand')

echo "Building..."
eval "$BUILD_CMD"

echo "Testing..."
eval "$TEST_CMD"
```

### Batch Analysis

Analyze multiple repositories:

```bash
for repo in repos/*; do
    echo "Analyzing $repo..."
    aipack detect "$repo" --format json > "results/$(basename $repo).json"
done
```

### CI/CD Integration

GitHub Actions:

```yaml
- name: Detect and build
  run: |
    DETECTION=$(aipack detect --format json)
    BUILD_CMD=$(echo "$DETECTION" | jq -r '.buildCommand')
    eval "$BUILD_CMD"
```

For more examples, see:
- [docs/EXAMPLES.md](docs/EXAMPLES.md) - Comprehensive usage examples
- [examples/](examples/) - Runnable code examples

## Supported Languages

aipack can detect build commands for projects in:

| Language   | Build Systems                    | Confidence |
|------------|----------------------------------|------------|
| Rust       | cargo                            | ✓✓✓        |
| JavaScript | npm, yarn, pnpm, bun             | ✓✓✓        |
| TypeScript | npm, yarn, pnpm (with tsc)       | ✓✓✓        |
| Java       | maven, gradle, ant               | ✓✓         |
| Kotlin     | gradle, maven                    | ✓✓         |
| Python     | pip, poetry, pipenv, setuptools  | ✓✓         |
| Go         | go mod, make                     | ✓✓✓        |
| Ruby       | bundler, rake, gem               | ✓✓         |
| PHP        | composer                         | ✓✓         |
| .NET       | dotnet, msbuild                  | ✓✓         |
| C/C++      | make, cmake, meson, ninja        | ✓          |
| Swift      | swift package manager, xcodebuild| ✓          |
| Scala      | sbt, mill                        | ✓✓         |

And many more! The AI-powered approach means aipack can handle any build system, even custom or proprietary ones.

## Documentation

- **[ARCHITECTURE.md](docs/ARCHITECTURE.md)** - System architecture and design
- **[DEVELOPMENT.md](docs/DEVELOPMENT.md)** - Development guide for contributors
- **[EXAMPLES.md](docs/EXAMPLES.md)** - Real-world usage examples
- **[TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md)** - Common issues and solutions
- **[CONFIGURATION_GUIDE.md](docs/CONFIGURATION_GUIDE.md)** - Complete configuration reference
- **[CONTRIBUTING.md](CONTRIBUTING.md)** - Contribution guidelines
- **[CHANGELOG.md](CHANGELOG.md)** - Version history
- **[PRD.md](PRD.md)** - Product requirements and vision

## Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/diverofdark/aipack.git
cd 

# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Check code quality
cargo clippy
cargo fmt
```

### Running Examples

```bash
# Basic detection
cargo run --example basic_detect

# Custom configuration
cargo run --example custom_config -- /path/to/repo

# Batch analysis
cargo run --example batch_analyze -- /path/to/repos

# Advanced workflow
RUST_LOG=debug cargo run --example advanced_workflow
```

### Testing

```bash
# All tests
cargo test

# Integration tests (requires Ollama)
cargo test --test '*'

# With coverage
cargo tarpaulin --out Html
```

For detailed development instructions, see [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## Performance

### Latency

| Backend          | Typical Time | Notes                    |
|------------------|--------------|--------------------------|
| Ollama (qwen2.5-coder:7b) | 1-3 seconds  | Local inference, no network |
| Ollama (qwen:14b)| 3-8 seconds  | Better accuracy, slower  |
| Mistral API      | 2-5 seconds  | Includes network latency |

### Resource Usage

| Model      | RAM Usage | Disk Space | CPU        |
|------------|-----------|------------|------------|
| qwen2.5-coder:7b    | ~4-6 GB   | ~4 GB      | Medium     |
| qwen:14b   | ~8-12 GB  | ~8 GB      | High       |
| qwen:32b   | ~20-24 GB | ~18 GB     | Very High  |

### Optimization Tips

- Use `qwen2.5-coder:7b` for fast detection
- Use `qwen:14b` for better accuracy
- Enable caching for repeated queries (future feature)
- Use JSON output for scripting (faster parsing)

## Troubleshooting

### Common Issues

**Ollama connection refused**
```bash
# Start Ollama
ollama serve

# Verify it's running
curl http://localhost:11434/api/tags
```

**Model not found**
```bash
# Pull the model
ollama pull qwen2.5-coder:7b

# List available models
ollama list
```

**Low confidence results**
- Ensure standard build configuration files exist
- Try a more powerful model (`qwen:14b`)
- Check if repository structure is non-standard

**Detection is slow**
- Use smaller model (`qwen2.5-coder:7b`)
- Check system resources (RAM, CPU)
- Verify Ollama is not swapping

For comprehensive troubleshooting, see [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md).

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for:

- Code of conduct
- Development setup
- Pull request process
- Coding standards
- Testing guidelines

Quick contribution workflow:

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes and add tests
4. Ensure tests pass: `cargo test`
5. Run quality checks: `cargo clippy && cargo fmt`
6. Submit a pull request

## Roadmap

### Phase 1 - MVP (Current)
- ✅ Ollama/Qwen backend
- ✅ Basic detection workflow
- ✅ JSON/YAML output
- ✅ CLI interface
- ✅ Confidence scoring
- ✅ Comprehensive documentation

### Phase 2 - Enhanced Detection
- ⏳ Claude API backend
- ⏳ OpenAI GPT backend
- ⏳ Result caching system
- ⏳ Custom model support
- ⏳ Improved prompt engineering
- ⏳ Multi-language monorepo support

### Phase 3 - Web Service & Advanced Features
- ⏳ HTTP/REST API service
- ⏳ Batch processing capabilities
- ⏳ Web UI dashboard
- ⏳ Docker integration templates
- ⏳ Kubernetes manifests
- ⏳ Performance optimization

### Future Considerations
- Learning from user feedback
- Community-contributed detection patterns
- Build command validation
- Interactive detection mode
- Plugin system for custom backends

## FAQ

**Q: Can aipack work offline?**
A: Yes! Use Ollama backend with pre-downloaded models for completely offline operation.

**Q: How much RAM do I need?**
A: Minimum 8GB for qwen2.5-coder:7b, 16GB recommended for qwen:14b.

**Q: Does aipack send my code to external servers?**
A: Only if using Mistral API backend. Ollama runs 100% locally with no external communication.

**Q: Can I use my own LLM?**
A: Yes! Implement the `LLMBackend` trait. See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for details.

**Q: Why is detection slow?**
A: LLM inference takes time (1-10 seconds). Use faster models or local Ollama for better performance.

**Q: Can I cache results?**
A: Not built-in yet (coming in Phase 2), but you can implement caching yourself. See [docs/EXAMPLES.md](docs/EXAMPLES.md).

**Q: What if detection is wrong?**
A: Check confidence score. Low confidence (<70%) indicates uncertainty. Try a more powerful model or verify results manually.

**Q: Can aipack detect custom build systems?**
A: Yes! The AI approach works with any build system, including proprietary or custom tools.

**Q: How do I report bugs?**
A: Open an issue on [GitHub Issues](https://github.com/diverofdark/aipack/issues) with:
  - Your environment (OS, Rust version, aipack version)
  - Steps to reproduce
  - Expected vs actual behavior
  - Logs (with `RUST_LOG=aipack=debug`)

**Q: Is aipack production-ready?**
A: Yes for MVP use cases. Comprehensive error handling, logging, and testing. Production integration features coming in Phase 2-3.


## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- LLM backends powered by [Qwen](https://huggingface.co/Qwen/) and [Mistral AI](https://mistral.ai/)
- Local inference via [Ollama](https://ollama.ai/)

## Support

- **GitHub Issues**: [Report bugs and request features](https://github.com/diverofdark/aipack/issues)
- **GitHub Discussions**: [Ask questions and share ideas](https://github.com/diverofdark/aipack/discussions)
- **Documentation**: Comprehensive guides in [docs/](docs/)
- **Examples**: Working code examples in [examples/](examples/)

---

**Made with ❤️ by Kirill Orlov**
