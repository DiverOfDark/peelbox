# peelbox

AI-powered BuildKit frontend for intelligent build detection with Wolfi-first containerization.

Automatically analyzes repositories and generates secure, minimal container images using Wolfi packages and BuildKit.

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

## Table of Contents

- [Overview](#overview)
- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [Usage](#usage)
  - [Detection](#detection)
  - [BuildKit Frontend](#buildkit-frontend)
  - [Building Images](#building-images)
- [Wolfi-First Architecture](#wolfi-first-architecture)
- [Distroless Images](#distroless-images)
- [Configuration](#configuration)
- [Examples](#examples)
- [Supported Languages](#supported-languages)
- [Documentation](#documentation)
- [Development](#development)
- [Troubleshooting](#troubleshooting)

## Overview

**peelbox** is a BuildKit frontend that eliminates friction when containerizing applications by:
- Automatically detecting build systems, runtimes, and dependencies
- Generating secure, minimal container images using Wolfi packages
- Producing distroless final images with 2-layer optimization
- Providing SBOM and provenance attestations for supply chain security

### Why peelbox?

- **Wolfi-First**: All images use secure, minimal Wolfi packages (not Debian/Ubuntu)
- **Distroless by Default**: Production-ready images without package managers or shells
- **BuildKit Native**: Direct LLB generation for optimal performance
- **Supply Chain Security**: Built-in SBOM and SLSA provenance support
- **Language Agnostic**: Works with any programming language or build system
- **Fast & Deterministic**: Static analysis first, LLM fallback only when needed

## Features

- **Wolfi-Only Base Images**: Secure, minimal Wolfi packages instead of traditional base images
- **Distroless Final Images**: 2-layer optimized images (~10-30MB) without package managers or shells
- **BuildKit Frontend**: Native LLB generation for optimal build performance
- **Context Transfer Optimization**: 99.995% reduction (1.5GB → ~100KB) via gitignore-based filtering
- **Multi-Backend LLM Support**: Ollama, Claude, OpenAI, Gemini, Groq, or embedded inference
- **Dynamic Version Discovery**: Automatically detects available Wolfi package versions
- **Package Validation**: Fuzzy matching and version-aware validation against Wolfi APKINDEX
- **13 Languages**: Rust, Java, Kotlin, JavaScript, TypeScript, Python, Go, C#, Ruby, PHP, C++, Elixir, F#
- **16 Build Systems**: Cargo, Maven, Gradle, npm, yarn, pnpm, Bun, pip, poetry, go mod, dotnet, composer, bundler, CMake, mix, pipenv
- **20 Frameworks**: Spring Boot, Quarkus, Next.js, Django, Rails, Actix-web, and more
- **SBOM & Provenance**: Supply chain security attestations (via BuildKit)

## Quick Start

### 1. Install BuildKit

peelbox requires BuildKit v0.11.0+ for image building:

```bash
# Verify BuildKit
docker buildx version

# Or install standalone buildkit
# macOS
brew install buildkit

# Linux
sudo apt install buildkit
```

### 2. Run peelbox

#### Option A: Use Docker Image (No Installation)

```bash
# Detect your project using the published Docker image
docker run --rm -v $(pwd):/workspace ghcr.io/diverofdark/peelbox:latest \
  detect /workspace > universalbuild.json

# View the generated build specification
cat universalbuild.json
```

#### Option B: Install from Source

```bash
# Build and install peelbox locally
git clone https://github.com/diverofdark/peelbox.git
cd peelbox
cargo build --release
sudo install -m 755 target/release/peelbox /usr/local/bin/

# Now use peelbox directly
peelbox detect . > universalbuild.json
```

### 3. Build your first distroless image

```bash
cd /path/to/your/project

# Start BuildKit daemon
docker run -d --rm --name buildkitd --privileged \
  -p 127.0.0.1:1234:1234 \
  moby/buildkit:latest --addr tcp://0.0.0.0:1234

# Generate LLB and build
PEELBOX_DETECTION_MODE=static cargo run --release -- frontend | \
  buildctl --addr tcp://127.0.0.1:1234 build \
    --local context=$(pwd) \
    --output type=docker,name=localhost/myapp:latest | \
  docker load

# Run your distroless image
docker run --rm localhost/myapp:latest

# Verify it's truly distroless
docker run --rm localhost/myapp:latest test -f /sbin/apk && echo "FAIL" || echo "PASS"
```

Example output from `peelbox detect`:

```json
{
  "version": "1.0",
  "metadata": {
    "project_name": "my-app",
    "language": "Rust",
    "build_system": "cargo",
    "confidence": 0.98
  },
  "build": {
    "packages": ["rust", "build-base"],
    "commands": ["cargo build --release"],
    "cache": ["/cache/cargo"],
    "artifacts": ["/build/target/release/my-app"]
  },
  "runtime": {
    "packages": [],
    "command": ["./my-app"],
    "ports": [8080]
  }
}
```

Note: No base images! peelbox uses `cgr.dev/chainguard/wolfi-base` automatically.

## Installation

### Option 1: Use Docker Image (Recommended)

No installation needed! Use the published Docker image:

```bash
# Pull the latest image
docker pull ghcr.io/diverofdark/peelbox:latest

# Run peelbox via Docker
docker run --rm -v $(pwd):/workspace \
  ghcr.io/diverofdark/peelbox:latest \
  detect /workspace
```

**Advantages:**
- No local installation required
- Always up-to-date with latest release
- Works on any platform with Docker
- Embedded LLM included (zero-config)

### Option 2: Install from Source

#### Prerequisites

- **Rust 1.70+**: [rustup.rs](https://rustup.rs/)
- **BuildKit v0.11.0+**: Docker Desktop 4.17+, Docker Engine 23.0+, or standalone buildkit
- **buildctl CLI**: Included with BuildKit installations
- **Ollama** (optional, for local LLM): [ollama.ai](https://ollama.ai/)

#### Build and Install

```bash
git clone https://github.com/diverofdark/peelbox.git
cd peelbox

# Build with embedded LLM (default, zero-config)
cargo build --release

# Or build with minimal features (Ollama/API only, smaller binary)
cargo build --release --no-default-features

# Install
sudo install -m 755 target/release/peelbox /usr/local/bin/
```

### Verify Installation

```bash
peelbox --version
buildctl --version  # Should be v0.11.0+
```

## Usage

### Detection

Analyze a repository and generate `UniversalBuild` specification:

```bash
# Detect current directory
peelbox detect .

# Detect specific repository
peelbox detect /path/to/repo

# Save to file
peelbox detect . > universalbuild.json

# JSON output (default)
peelbox detect . --format json

# Human-readable display
peelbox detect .
```

Detection output includes:
- Language, build system, and framework
- Wolfi packages for build and runtime stages
- Build commands and environment variables
- Cache directories and artifacts
- Runtime configuration (ports, health checks, environment)

### BuildKit Frontend

Generate LLB (Low-Level Build) definition from UniversalBuild spec:

```bash
# Generate LLB to stdout
peelbox frontend --spec universalbuild.json

# Pipe directly to buildctl
peelbox frontend --spec universalbuild.json | buildctl build ...
```

The frontend command:
- Reads `UniversalBuild` JSON specification
- Generates BuildKit LLB protobuf
- Applies gitignore-based context filtering (99.995% reduction)
- Creates 2-stage distroless build graph

### Building Images

#### Complete Build Workflow

**Step 1: Generate LLB**

```bash
cd /path/to/your/project

# Generate LLB using static detection (fast, no LLM needed)
PEELBOX_DETECTION_MODE=static cargo run --release -- frontend > /tmp/llb.pb

# Or use full detection with LLM for unknown build systems
cargo run --release -- frontend > /tmp/llb.pb
```

**Step 2: Start BuildKit daemon**

```bash
# Start BuildKit in Docker container
docker run -d --rm --name buildkitd --privileged \
  -p 127.0.0.1:1234:1234 \
  moby/buildkit:latest --addr tcp://0.0.0.0:1234

# BuildKit is now listening on tcp://127.0.0.1:1234
```

**Step 3: Build with buildctl**

```bash
# Build and export to tar
cat /tmp/llb.pb | buildctl --addr tcp://127.0.0.1:1234 build \
  --local context=/path/to/your/project \
  --output type=docker,name=localhost/myapp:latest > /tmp/myapp.tar

# Load into Docker
docker load < /tmp/myapp.tar

# Or pipe directly to docker load
cat /tmp/llb.pb | buildctl --addr tcp://127.0.0.1:1234 build \
  --local context=/path/to/your/project \
  --output type=docker,name=localhost/myapp:latest | docker load
```

**Step 4: Verify distroless image**

```bash
# Run your application
docker run --rm localhost/myapp:latest

# Verify no package manager (should fail)
docker run --rm localhost/myapp:latest test -f /sbin/apk && echo "FAIL" || echo "PASS"

# Check no wolfi-base in history (should output nothing)
docker history localhost/myapp:latest | grep wolfi-base

# View clean layer metadata
docker history localhost/myapp:latest --format "table {{.Size}}\t{{.CreatedBy}}"
```

Expected output:
```
SIZE      CREATED BY
16.2MB    sh -c : peelbox myapp application && ...
10.2MB    sh -c : peelbox glibc ca-certificates runtime; ...
...       pulled from cgr.dev/chainguard/glibc-dynamic:latest
```

#### With SBOM and Provenance

```bash
cat /tmp/llb.pb | buildctl --addr tcp://127.0.0.1:1234 build \
  --local context=/path/to/your/project \
  --output type=docker,name=localhost/myapp:latest \
  --opt attest:sbom= \
  --opt attest:provenance=mode=max \
  | docker load
```

#### One-Liner (for automation)

```bash
PEELBOX_DETECTION_MODE=static cargo run --release -- frontend | \
  buildctl --addr tcp://127.0.0.1:1234 build \
    --local context=$(pwd) \
    --output type=docker,name=localhost/myapp:latest | \
  docker load
```

## Wolfi-First Architecture

peelbox uses **Wolfi packages exclusively** for all container images:

### What is Wolfi?

[Wolfi](https://github.com/wolfi-dev) is a Linux distribution purpose-built for containers:
- Minimal attack surface (only necessary packages)
- glibc-based (compatible with most applications)
- Daily security updates
- APK package manager (same as Alpine)
- Maintained by Chainguard

### Wolfi Package Examples

Common Wolfi packages peelbox uses:

| Purpose | Wolfi Package | Notes |
|---------|---------------|-------|
| Rust toolchain | `rust` | Latest stable Rust |
| Node.js 22 runtime | `nodejs-22` | Version-specific packages |
| Node.js 20 runtime | `nodejs-20` | Multiple versions available |
| Python 3.12 | `python-3.12` | Version-specific |
| Java 21 JDK | `openjdk-21` | Full JDK |
| Java 21 JRE | `openjdk-21-jre` | Runtime only (smaller) |
| Go toolchain | `go` | Latest Go |
| Build essentials | `build-base` | gcc, make, etc. |
| SSL/TLS support | `openssl` | OpenSSL library |
| CA certificates | `ca-certificates` | Trusted root CAs |

### Dynamic Version Discovery

peelbox automatically discovers available Wolfi package versions:

```bash
# Fetches APKINDEX from packages.wolfi.dev
# Caches for 24 hours (binary cache for 30x performance)
# Selects best version match for your project

# Example: package.json specifies Node 20
# peelbox automatically selects nodejs-20 from Wolfi
```

### Package Validation

All packages are validated against Wolfi APKINDEX with fuzzy matching:

```bash
✓ Valid: nodejs-22, python-3.12, openjdk-21
✗ Invalid: nodejs → Error: "Did you mean: nodejs-22, nodejs-20, nodejs-18?"
✗ Invalid: pythonn-3.12 → Error: "Did you mean: python-3.12?"
```

## Distroless Images

**All peelbox images are distroless by default** - no opt-out, no flag needed.

### What is Distroless?

Distroless images contain only:
- Your application binary
- Runtime dependencies (libraries)
- Minimal Wolfi runtime files

They do NOT contain:
- Package managers (`/sbin/apk`)
- Shell (`/bin/sh`, `/bin/bash`)
- Package databases (`/var/lib/apk`)
- Build tools or unnecessary utilities

### Squashed Distroless Architecture

peelbox generates truly distroless images with **no wolfi-base in layer history**:

```
Final Image Layers:
Layer 1-5: glibc-dynamic:latest (~11MB)
  - Clean distroless base (no apk ever existed)

Layer 6: Squashed Runtime (~10MB)
  - Runtime packages (glibc, ca-certificates, etc.)
  - Package manager removed (no /sbin/apk)
  - Clean metadata: ": peelbox <packages> runtime"

Layer 7: Application (~16MB)
  - Your compiled binary/artifacts
  - Clean metadata: ": peelbox <name> application"

Total: ~13MB (peelbox example)
```

### Build Process

```
Stage 1 (Build):
  wolfi-base + build packages → build app → artifacts

Stage 2 (Runtime Prep):
  wolfi-base + runtime packages → remove apk

Stage 3 (Squash to Clean Base):
  glibc-dynamic (clean, no apk) + copy runtime prep → squashed layer

Stage 4 (Final):
  squashed runtime + copy artifacts → final image
```

**Result**: No apk in filesystem, no wolfi-base in history - truly distroless.

### Benefits

- **True Distroless**: No package manager in any layer (including history)
- **Security**: No attack surface from shells or package managers
- **Clean History**: No wolfi-base layers (only glibc-dynamic)
- **Size**: Optimized ~13MB total for Rust apps
- **Performance**: Faster container starts, less network transfer
- **Layer Metadata**: Clean descriptions for debugging
- **Production-Ready**: Industry best practice (Google Distroless, Chainguard)

### Verification

```bash
# Verify no apk in filesystem
docker run --rm myapp:latest test -f /sbin/apk && echo "FAIL" || echo "PASS"

# Verify no wolfi-base in history
docker history myapp:latest | grep wolfi-base && echo "FAIL" || echo "PASS"

# View clean layer metadata
docker history myapp:latest --format "table {{.Size}}\t{{.CreatedBy}}"
```

## Configuration

### Environment Variables

```bash
# LLM Provider Selection
export PEELBOX_PROVIDER=ollama       # "ollama", "claude", "openai", "gemini", "grok", "groq", "embedded"

# Model Configuration
export PEELBOX_MODEL=qwen2.5-coder:7b

# Request Configuration
export PEELBOX_REQUEST_TIMEOUT=60    # seconds
export PEELBOX_MAX_TOKENS=8192       # max response tokens

# Detection Mode
export PEELBOX_DETECTION_MODE=full   # "full", "static", or "llm"

# Embedded Model Size (auto-selected by default)
export PEELBOX_MODEL_SIZE=7B         # "0.5B", "1.5B", "3B", "7B"

# Logging
export RUST_LOG=peelbox=info         # debug, info, warn, error

# Provider-Specific
export OLLAMA_HOST=http://localhost:11434
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...
export GOOGLE_API_KEY=AIza...
export XAI_API_KEY=xai-...
export GROQ_API_KEY=gsk_...
```

### Detection Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `full` | Static analysis + LLM fallback (default) | Normal operation, best accuracy |
| `static` | Static analysis only, no LLM | Fast CI tests, deterministic detection |
| `llm` | LLM-only detection | Test LLM prompts specifically |

### LLM Provider Selection

peelbox auto-selects the best available LLM:

1. Check `PEELBOX_PROVIDER` environment variable
2. Try connecting to Ollama (localhost:11434)
3. Fall back to embedded local inference (zero-config)

```bash
# Auto-select (tries Ollama, falls back to embedded)
peelbox detect .

# Force specific provider
PEELBOX_PROVIDER=ollama peelbox detect .
PEELBOX_PROVIDER=claude ANTHROPIC_API_KEY=sk-... peelbox detect .
PEELBOX_PROVIDER=embedded peelbox detect .
```

## Examples

### Using Docker Image (No Installation)

```bash
cd myproject

# 1. Detect using Docker image
docker run --rm -v $(pwd):/workspace \
  ghcr.io/diverofdark/peelbox:latest \
  detect /workspace > universalbuild.json

# 2. Start BuildKit daemon
docker run -d --rm --name buildkitd --privileged \
  -p 127.0.0.1:1234:1234 \
  moby/buildkit:latest --addr tcp://0.0.0.0:1234

# 3. Generate LLB and build
docker run --rm -v $(pwd):/workspace \
  ghcr.io/diverofdark/peelbox:latest \
  frontend --spec /workspace/universalbuild.json | \
  buildctl --addr tcp://127.0.0.1:1234 build \
    --local context=$(pwd) \
    --output type=docker,name=myapp:latest | \
  docker load

# 4. Run your distroless image
docker run -p 8080:8080 myapp:latest

# 5. Verify it's truly distroless
docker run --rm myapp:latest test -f /sbin/apk && echo "FAIL" || echo "PASS"
```

### Basic Workflow (Installed Binary)

```bash
# 1. Detect build configuration
cd myproject
peelbox detect . > universalbuild.json

# 2. Generate LLB and build
peelbox frontend --spec universalbuild.json | \
  buildctl build \
    --local context=. \
    --output type=docker,name=myapp:latest

# 3. Run the image
docker run -p 8080:8080 myapp:latest
```

### With SBOM and Provenance

```bash
peelbox frontend | buildctl build \
  --local context=. \
  --output type=docker,name=myapp:latest \
  --opt attest:sbom= \
  --opt attest:provenance=mode=max \
  --opt build-arg:BUILDKIT_SBOM_SCAN_CONTEXT=true

# View SBOM
docker buildx imagetools inspect myapp:latest \
  --format '{{json .SBOM}}'
```

### CI/CD Integration (GitHub Actions)

```yaml
name: Build Container
on: push

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up BuildKit
        uses: docker/setup-buildx-action@v3

      - name: Install peelbox
        run: |
          curl -L https://github.com/yourusername/peelbox/releases/latest/download/peelbox-linux-amd64 -o peelbox
          chmod +x peelbox

      - name: Detect and build
        run: |
          ./peelbox detect . > universalbuild.json
          ./peelbox frontend --spec universalbuild.json | \
            buildctl build \
              --local context=. \
              --output type=docker,name=ghcr.io/${{ github.repository }}:${{ github.sha }},push=true \
              --opt attest:sbom= \
              --opt attest:provenance=mode=max
```

### Context Transfer Optimization

peelbox automatically reduces context transfer by 99.995%:

```bash
# Before optimization: 1.54GB context transfer
# After optimization: 80KB-113KB (99.995% reduction)

# Uses .gitignore patterns + standard exclusions:
# - .git/
# - target/, node_modules/, build/
# - *.md, LICENSE, README
# - .vscode/, .idea/

# No manual configuration needed!
```

For more examples:
- [docs/EXAMPLES.md](docs/EXAMPLES.md) - Comprehensive usage examples
- [docs/SBOM_AND_PROVENANCE.md](docs/SBOM_AND_PROVENANCE.md) - SBOM/provenance guide
- [examples/](examples/) - Runnable code examples

## Supported Languages

| Language   | Build Systems        | Wolfi Packages                                    | Confidence |
|------------|----------------------|---------------------------------------------------|------------|
| Rust       | cargo                | `rust`, `build-base`                              | ✓✓✓        |
| JavaScript | npm, yarn, pnpm, bun | `nodejs-22`, `nodejs-20`                          | ✓✓✓        |
| TypeScript | npm, yarn, pnpm      | `nodejs-22`                                       | ✓✓✓        |
| Java       | maven, gradle        | `openjdk-21`, `openjdk-21-jre`, `maven`, `gradle` | ✓✓✓        |
| Kotlin     | gradle, maven        | `openjdk-21`, `gradle`                            | ✓✓         |
| Python     | pip, poetry, pipenv  | `python-3.12`, `py3-pip`                          | ✓✓✓        |
| Go         | go mod               | `go`, `build-base`                                | ✓✓✓        |
| C# / F#    | dotnet               | `dotnet-8`, `dotnet-8-runtime`                    | ✓✓         |
| Ruby       | bundler              | `ruby-3.3`, `bundler`                             | ✓✓         |
| PHP        | composer             | `php-8.3`, `composer`                             | ✓✓         |
| C++        | cmake, make          | `build-base`, `cmake`                             | ✓✓         |
| Elixir     | mix                  | `elixir`, `erlang`                                | ✓✓         |

## Documentation

- **[SBOM_AND_PROVENANCE.md](docs/SBOM_AND_PROVENANCE.md)** - Supply chain security guide
- **[CLAUDE.md](CLAUDE.md)** - Development guide for AI assistants
- **[ARCHITECTURE.md](docs/ARCHITECTURE.md)** - System architecture and design
- **[CHANGELOG.md](CHANGELOG.md)** - Version history
- **[PRD.md](PRD.md)** - Product requirements and vision

## Development

### Building from Source

```bash
git clone https://github.com/diverofdark/peelbox.git
cd peelbox

# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run e2e tests (static mode, no LLM needed)
cargo test --test e2e static

# Run integration tests (requires Docker/Podman)
cargo test --test buildkit_integration -- --ignored --nocapture

# Code quality
cargo clippy
cargo fmt
```

### Running Tests

```bash
# Unit tests (fast)
cargo test --lib

# E2E tests in static mode (deterministic, no LLM)
PEELBOX_DETECTION_MODE=static cargo test --test e2e

# E2E tests with LLM (requires Ollama or embedded model)
cargo test --test e2e

# Integration tests (requires Docker/Podman + BuildKit)
cargo test --test buildkit_integration -- --ignored --nocapture
```

## Troubleshooting

### BuildKit Issues

**BuildKit not available:**
```bash
# Check version
buildctl --version  # Should be v0.11.0+

# Start buildkitd
docker run -d --name buildkitd --privileged moby/buildkit:latest

# Or use Docker BuildKit
export DOCKER_BUILDKIT=1
```

**Context transfer too slow:**
```bash
# peelbox automatically applies gitignore filtering
# If still slow, check .gitignore includes build artifacts
echo "target/" >> .gitignore
echo "node_modules/" >> .gitignore
```

### LLM Issues

**Ollama connection refused:**
```bash
ollama serve  # Start Ollama
ollama pull qwen2.5-coder:7b  # Pull model
```

**Use embedded model (zero-config):**
```bash
PEELBOX_PROVIDER=embedded peelbox detect .
# Auto-selects model based on available RAM
```

### Package Validation Errors

**Package not found:**
```bash
# peelbox suggests alternatives
Error: Package 'nodejs' not found. Did you mean: nodejs-22, nodejs-20, nodejs-18?

# Use version-specific package
build.packages = ["nodejs-22"]
```

## Acknowledgments

- Built with [Rust](https://www.rust-lang.org/)
- Container images powered by [Wolfi](https://github.com/wolfi-dev) and [Chainguard](https://www.chainguard.dev/)
- BuildKit integration via [buildkit-llb](https://crates.io/crates/buildkit-llb)
- Local LLM inference with [Qwen2.5-Coder](https://github.com/QwenLM/Qwen2.5-Coder) (GGUF format)

## Support

- **GitHub Issues**: [Report bugs and request features](https://github.com/diverofdark/peelbox/issues)
- **Documentation**: Comprehensive guides in [docs/](docs/)

---

**Secure, minimal, production-ready containers - automatically.**
