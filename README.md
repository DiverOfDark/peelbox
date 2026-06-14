# peelbox

Deterministic BuildKit frontend for intelligent build detection with Wolfi-first containerization.

Automatically analyzes repositories and generates secure, minimal container images using Wolfi packages and BuildKit. Detection is fully static and reproducible — no API keys, no network LLM calls, byte-identical output across runs.

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
- **Fast & Deterministic**: Pure static analysis — reproducible, offline, no API keys or model downloads

## Features

- **Wolfi-Only Base Images**: Secure, minimal Wolfi packages instead of traditional base images
- **Distroless Final Images**: 2-layer optimized images (~10-30MB) without package managers or shells
- **BuildKit Frontend**: Native LLB generation for optimal build performance
- **Context Transfer Optimization**: 99.995% reduction (1.5GB → ~100KB) via gitignore-based filtering
- **Deterministic Detection**: Pure static manifest/config analysis — no LLM, no network, reproducible output
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

# Build image directly (auto-detects build system and connects to BuildKit)
# If using Docker Desktop, it works out of the box.
# If using standalone BuildKit:
peelbox build --tag localhost/myapp:latest --buildkit tcp://127.0.0.1:1234

# Run your distroless image
docker run --rm localhost/myapp:latest

# Verify it's truly distroless (no apk, no shell)
docker run --rm localhost/myapp:latest /sbin/apk --version && echo "FAIL" || echo "PASS"
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
- Zero-config: no API keys, no model downloads, no network for detection

### Option 2: Install from Source

#### Prerequisites

- **Rust 1.70+**: [rustup.rs](https://rustup.rs/)
- **BuildKit v0.11.0+**: Docker Desktop 4.17+, Docker Engine 23.0+, or standalone buildkit

#### Build and Install

```bash
git clone https://github.com/diverofdark/peelbox.git
cd peelbox

# Build
cargo build --release

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

### Building Images

Build container images directly from UniversalBuild spec (no buildctl required):

```bash
# Build using local BuildKit daemon (auto-detects Docker or standalone)
peelbox build --spec universalbuild.json --tag myapp:latest

# Build specific service in monorepo
peelbox build --spec universalbuild.json --tag api:latest --service api

# Output to OCI tarball instead of Docker daemon
peelbox build --spec universalbuild.json --tag myapp:latest --output type=oci,dest=image.tar
```

The build command:
- Connects directly to BuildKit (gRPC)
- Transfers build context efficiently (gitignore-aware)
- Streams real-time build progress
- Generates SBOM and Provenance attestations by default
- Loads result into Docker daemon automatically (default) or exports to file

#### Complete Workflow

```bash
cd /path/to/your/project

# 1. Detect build configuration
peelbox detect . > universalbuild.json

# 2. Build image
peelbox build --spec universalbuild.json --tag localhost/myapp:latest

# 3. Run it
docker run --rm localhost/myapp:latest
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

Detection is fully deterministic and requires no configuration — no API keys,
no providers, no models. The few supported environment variables tune logging
and caching:

```bash
# Logging
export RUST_LOG=peelbox=info         # debug, info, warn, error

# Wolfi APKINDEX + build cache location (defaults to a temp dir)
export PEELBOX_CACHE_DIR=/path/to/cache
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

# 3. Build the image
docker run --rm -v $(pwd):/workspace \
  ghcr.io/diverofdark/peelbox:latest \
  build --spec /workspace/universalbuild.json \
    --tag myapp:latest \
    --context /workspace \
    --buildkit tcp://127.0.0.1:1234

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

# 2. Build the image
peelbox build --spec universalbuild.json --tag myapp:latest

# 3. Run the image
docker run -p 8080:8080 myapp:latest
```

### With SBOM and Provenance

SBOM and SLSA provenance attestations are generated by default:

```bash
peelbox build --spec universalbuild.json --tag myapp:latest \
  --provenance max

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
          ./peelbox build \
            --spec universalbuild.json \
            --tag ghcr.io/${{ github.repository }}:${{ github.sha }}
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
- [docs/SBOM_AND_PROVENANCE.md](docs/SBOM_AND_PROVENANCE.md) - SBOM/provenance guide

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
- **[BUILDKIT_GRPC_LLB_TAR_EXPORT_FLOW.md](docs/BUILDKIT_GRPC_LLB_TAR_EXPORT_FLOW.md)** - BuildKit gRPC/LLB internals
- **[AGENTS.md](AGENTS.md)** - Development guide and repository conventions

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

# Run e2e tests (deterministic, fixture-driven)
cargo test --test static_e2e

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

# Static detection E2E tests (deterministic, fixture-driven)
cargo test --test static_e2e

# Container build tests (requires Docker/Podman + BuildKit)
cargo test --test container_e2e
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

## Support

- **GitHub Issues**: [Report bugs and request features](https://github.com/diverofdark/peelbox/issues)
- **Documentation**: Comprehensive guides in [docs/](docs/)

---

**Secure, minimal, production-ready containers - automatically.**
