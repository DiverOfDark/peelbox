# aipack - Product Requirements Document

## Executive Summary

**aipack** is an AI-powered buildkit frontend that intelligently detects and recommends build commands for repositories using LLM analysis. It leverages Mistral API and/or locally-running Qwen models (via Ollama) to analyze repository structure and generate appropriate build commands. aipack operates as a standalone developer tool for seamless integration into any development workflow.

## Vision

Eliminate the friction of building unfamiliar projects by leveraging AI to automatically detect build systems and generate correct build commands for any repository, without hardcoded detection logic.

## Problem Statement

Developers frequently encounter challenges when working with new repositories:
- **Discovery**: What build system does this project use?
- **Tooling**: What commands build, test, or deploy this code?
- **Variations**: Are there project-specific build steps?
- **Documentation**: Build instructions may be outdated or incomplete

Current solutions require:
- Manual inspection of package.json, Dockerfile, Makefile, etc.
- Reading README files
- Trial and error
- Context switching

aipack eliminates this friction through intelligent LLM-based analysis.

## Goals

1. **Primary Goal**: Use LLM to detect build systems and generate correct build commands with >90% accuracy
2. **Flexible AI Backends**: Support both Mistral API and locally-running Qwen (Ollama)
3. **Standalone Utility**: Valuable tool for any developer working with unfamiliar projects
4. **OSS Community**: Maintain as open-source project with clear contribution model
5. **Performance**: Fast analysis and generation (tuned for local inference when possible)
6. **Cost-Efficient**: Support offline/local inference to minimize API costs

## Key Principles

### AI-First Approach
- **No Hardcoded Detection**: Always use LLM analysis, not pattern matching
- **Flexible Models**: Support Mistral API (cloud) and Qwen (local Ollama)
- **Fallback Strategy**: Graceful degradation when AI unavailable (return empty/error)
- **Repository Context**: Analyze all relevant files for comprehensive understanding

### Flexible Architecture
- **Model Agnostic**: Design to work with multiple LLM providers
- **Configurable Backends**: Switch between Mistral and Ollama at runtime
- **Cost Optimization**: Prefer local inference by default, cloud as fallback

## Features

### MVP (Phase 1)
- **LLM-based Detection**: Use Mistral or Ollama to analyze repository
- **Repository Analysis**:
  - File structure mapping
  - Configuration file content analysis (package.json, Cargo.toml, build.gradle, pyproject.toml, etc.)
  - README and documentation parsing
  - Git history analysis (optional)
- **Command Generation**:
  - Primary build command
  - Test/verification command
  - Deploy/release command
  - Development/watch command
- **CLI Interface**:
  - `aipack detect <repo-path>` - auto-detect AI backend
  - `aipack detect --ollama <repo-path>` - use local Ollama
  - `aipack detect --mistral <repo-path>` - use Mistral API
- **Output Formats**: JSON, YAML, human-readable text
- **Configuration**: Environment-based config for API keys and Ollama endpoint

### Phase 2: Enhancements
- **Confidence Scoring**: LLM provides confidence assessment
- **Explanation Generation**: Why these commands were chosen
- **Template System**: User-defined context templates
- **Caching**: Smart caching to avoid redundant API calls
- **Multi-language Support**: Handle monorepos with multiple build systems

### Phase 3: Web Service & Platform Integration
- **HTTP Service**: REST/gRPC endpoint for build command detection
- **Web UI**: Simple dashboard for batch repository analysis
- **Caching Layer**: Persistent result caching for repeated queries
- **Batch Processing**: Analyze multiple repositories efficiently
- **Webhook Support**: Integration with GitHub/GitLab webhooks

### Future Enhancements
- **Performance Optimization**: Native compilation and distribution
- **Model Fine-tuning**: Custom models for specific domains
- **Plugin System**: Allow community LLM providers
- **Learning from Feedback**: Improve models based on user corrections
- **Container Support**: Docker images and Kubernetes integration

## AI/LLM Integration

### Supported Backends

#### 1. Mistral API
- **Endpoint**: `https://api.mistral.ai/v1`
- **Model**: `mistral-small` (fast) or `mistral-medium` (accurate)
- **Authentication**: `MISTRAL_API_KEY` environment variable
- **Use Case**: Cloud-based, always available, good for CI/CD
- **Cost**: Pay-per-use

#### 2. Local Ollama (Qwen)
- **Endpoint**: `http://localhost:11434` (configurable)
- **Models**:
  - `qwen2.5-coder:7b` (small, fast)
  - `qwen:14b` (larger, more accurate)
  - Any Ollama-compatible model
- **Authentication**: None (local)
- **Use Case**: Offline, no API costs, instant
- **Prerequisites**: Ollama installed and running

### Architecture
```
User/Platform
    ↓
aipack CLI or Service
    ↓ (configure backend)
┌─────────────────────────────────────┐
│  Backend Selection                  │
├─────────────────────────────────────┤
│ ✓ Use Ollama if available locally   │
│ ✓ Fallback to Mistral if configured │
│ ✗ Error if no backend available     │
└─────────────────────────────────────┘
    ↓
Repository Analysis
    │ (gather file structure & content)
    ↓
LLM Request
    │ (prompt engineering for build detection)
    ↓
Build Command Generation
    │ (parse and validate response)
    ↓
Output
```

### Prompt Design

```
You are a software engineering expert specializing in build systems and project configurations.

Analyze the following repository and identify the build/deployment commands:

REPOSITORY STRUCTURE:
[file tree]

KEY CONFIGURATION FILES:
[contents of relevant files]

README (if present):
[readme content]

Based on this analysis, identify:
1. Primary build system and language
2. Build command to compile/bundle the project
3. Test command to run tests
4. Deploy/release command for production
5. Any development/watch commands
6. Your confidence level (0-1) in these recommendations
7. Brief reasoning for your choices

Respond with valid JSON:
{
  "buildSystem": "string",
  "language": "string",
  "buildCommand": "string",
  "testCommand": "string",
  "deployCommand": "string",
  "devCommand": "string (optional)",
  "confidence": 0.0-1.0,
  "reasoning": "string",
  "warnings": ["list of potential issues"]
}
```

## Standalone Usage

### Installation
```bash
cargo install aipack
```

### Configuration
```bash
# Use Ollama (default if running)
export AIPACK_BACKEND=ollama
export AIPACK_OLLAMA_ENDPOINT=http://localhost:11434

# OR use Mistral
export AIPACK_BACKEND=mistral
export MISTRAL_API_KEY=your-api-key
export AIPACK_MISTRAL_MODEL=mistral-small

# OR let aipack auto-detect
# (tries Ollama first, falls back to Mistral if configured)
```

### CLI Commands
```bash
# Detect build commands in current directory
aipack detect

# Detect in specific directory
aipack detect /path/to/repo

# Force specific backend
aipack detect --backend ollama /path/to/repo
aipack detect --backend mistral /path/to/repo

# Output format
aipack detect --format json
aipack detect --format yaml
aipack detect --format human

# Include verbose output
aipack detect --verbose

# Custom model (Ollama only)
aipack detect --model qwen:14b

# Set API key inline (not recommended)
MISTRAL_API_KEY=... aipack detect --backend mistral
```

### Output Example
```json
{
  "buildSystem": "cargo",
  "language": "Rust",
  "buildCommand": "cargo build --release",
  "testCommand": "cargo test",
  "deployCommand": "cargo build --release --target x86_64-unknown-linux-gnu",
  "confidence": 0.96,
  "reasoning": "Repository contains Cargo.toml with dependencies, src/main.rs entry point, and standard Rust project structure.",
  "warnings": [],
  "detectedFiles": ["Cargo.toml", "Cargo.lock", "src/main.rs", ".github/workflows/"]
}
```

## Future Web Service & API

### Architecture (Phase 3+)
```
Client Application
    ↓ REST/HTTP call
aipack Service
    ├─ Ollama Pod (optional, local inference)
    └─ Mistral API client (cloud fallback)
```

### Proposed API Endpoint
```
POST /api/v1/detect

Request:
{
  "repositoryPath": "/path/to/repo",
  "backend": "auto" | "ollama" | "mistral",
  "cacheResults": true
}

Response:
{
  "buildSystem": "gradle",
  "buildCommand": "gradle build",
  "testCommand": "gradle test",
  "deployCommand": "gradle build -Denv=prod",
  "confidence": 0.92,
  "reasoning": "Detected gradle wrapper and build.gradle.kts",
  "detectedFiles": ["build.gradle.kts", "gradle.properties"],
  "processingTimeMs": 2340
}
```

### Integration Opportunities
- CI/CD systems: Use detected commands in build pipelines
- IDE plugins: Integrate detection into development environments
- Documentation: Auto-generate build instructions
- Container orchestration: Kubernetes deployment manifests

## Non-Functional Requirements

### Performance
- **Ollama (local)**: 1-5s depending on model and repo size
- **Mistral API**: 2-10s including network latency
- **Caching**: Subsequent queries <50ms

### Reliability
- **Graceful Degradation**: Clear error messages when AI unavailable
- **Timeout Handling**: Configurable timeouts for API calls
- **Retry Logic**: Exponential backoff for transient failures

### Security
- **API Key Management**: Support env vars only (never hardcoded)
- **No Credential Leakage**: Never log sensitive data
- **SBOM**: Track dependencies for security audits

### Maintainability
- **Code Quality**: Rust idioms, minimal unsafe code
- **Testing**: >80% code coverage
- **Documentation**: Inline docs + user guides + examples

## Success Metrics

1. **Detection Accuracy**: >90% correct command generation
2. **User Adoption**: Adopted by major open-source projects
3. **Performance**: P95 latency <5s with cloud API, <2s with local Ollama
4. **Reliability**: 99.5% uptime for production deployments
5. **Community**: Active contributions from 10+ external developers
6. **Cost**: Users prefer local Ollama inference to reduce API costs

## Timeline

- **Phase 1 (MVP)**: 3-4 weeks - Core detection with Mistral + Ollama support (COMPLETED)
- **Phase 2 (Enhancements)**: 2-3 weeks - Caching, confidence scoring, Claude/OpenAI backends
- **Phase 3 (Web Service)**: 2-3 weeks - HTTP service, Web UI, batch processing
- **Ongoing**: Community contributions and feedback integration

## Open Questions

1. Which Ollama models provide best accuracy/speed trade-off?
2. Should caching use local SQLite or external services?
3. Support for private/authenticated repositories?
4. Monorepo handling (detect multiple build systems)?
5. Integration with container runtimes (Docker, Podman)?
6. Performance profiling and optimization priorities?
