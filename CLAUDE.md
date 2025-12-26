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

## Claude Rules - CRITICAL

**RULE 0 (MOST IMPORTANT):**
- **NEVER keep code for backwards compatibility** – breaking changes are preferred
- **ALWAYS remove dead code** – if you find unused code, delete it immediately
- If unsure whether code is needed → ASK, don't assume it should stay

The following rules are MANDATORY for CLAUDE:
 - Keep comments to the minimum, only in cases when it's required. No examples.
 - Don't keep code for backwards compatibility – remove it
 - Never postpone a task, never cut corners
 - No planned work is optional. There should be a valid technical reason for it.
 - Code simplicity is most important.
 - Dead code is a smell. Remove it, unless you think it will be required later – then ask the user whether it should be kept.

## Decision Checkpoints

Before making ANY of these decisions, STOP and re-read the Claude Rules above:
- Keeping old code/APIs/files "for compatibility"
- Marking tasks as "SKIPPED" or "OPTIONAL"
- Deciding "this can be done later"
- Choosing not to remove dead code
- Preserving deprecated functionality

If you're about to do any of these → You're probably violating a rule → ASK THE USER FIRST.

## When You MUST Ask the User

ALWAYS ask before:
1. Skipping any planned task (even if it seems unnecessary)
2. Keeping old code instead of removing it
3. Marking work as "backward compatible" or "optional"
4. Deciding a breaking change is "too risky"
5. Finding dead code and thinking "maybe someone uses this"

Default answer: REMOVE IT. Only keep if user explicitly says to.

## Common Mistakes to Avoid

### ❌ WRONG: "I'll keep the old API for backward compatibility"
### ✅ RIGHT: Remove old API, update all callers

### ❌ WRONG: "Tests use FrameworkRegistry, so I can't remove it"
### ✅ RIGHT: Update tests to use StackRegistry, then remove FrameworkRegistry

### ❌ WRONG: "This task seems optional, I'll mark it SKIPPED"
### ✅ RIGHT: Complete the task OR ask user if it should be skipped

### ❌ WRONG: "OpenSpec says 'minimal changes', so I'll keep old code"
### ✅ RIGHT: Project-specific rules (CLAUDE.md) override general guidelines

## Before Marking Work Complete

Run this checklist:
- [ ] Did I remove ALL dead/old code? (No files named *_old, *_legacy, or unused registries)
- [ ] Did I complete ALL tasks in tasks.md? (No SKIPPED items without user approval)
- [ ] Did I make any "backward compatibility" decisions? (If yes, WRONG - remove them)
- [ ] Are all tests passing? (Not just "most tests")
- [ ] Did I ask the user about ANY uncertainty? (Don't assume, ask)

If ANY checkbox fails → You violated a rule → Fix it before claiming completion.

## Development Policy

**IMPORTANT PRINCIPLES:**
1. **No Backwards Compatibility**: Breaking changes are acceptable and preferred when they improve the codebase. Never maintain compatibility with old APIs, configurations, or interfaces.
2. **No Historical Comments**: Code and documentation should reflect the current state only. Never include comments explaining what was added, removed, or changed (e.g., "removed X because...", "added Y to replace...").
3. **Clean Slate**: When refactoring, completely remove old code and update all references. The codebase should read as if it was always implemented the current way.
4. **Minimal Comments**: Keep commenting to a minimum. If code is simple and obvious it doesn't require comments. This is not a library so examples are not required.

## Project Overview

**aipack** is a Rust-based AI-powered buildkit frontend for intelligent build command detection. It uses LLM function calling with iterative tool execution to analyze repositories on-demand, avoiding context window limitations.

**Architecture**: Multi-phase pipeline with deterministic analysis
- 9-phase sequential pipeline orchestrated by code, not LLM
- Deterministic parsers for known formats (package.json, Cargo.toml, etc.)
- LLM used only for unknowns with minimal context (~150-500 tokens per prompt)
- Scales to large repositories with predictable token usage (1k-6k total)

**Key Tech Stack:**
- **Language**: Rust 1.70+
- **Build System**: Cargo (standard commands: `build`, `test`, `fmt`, `clippy`)
- **Container Platform**: BuildKit (LLB generation for distroless Wolfi images)
- **AI Backends**: GenAI (unified multi-provider client)
  - Ollama (local inference)
  - Anthropic Claude
  - OpenAI GPT
  - Google Gemini
  - xAI Grok
  - Groq
  - Embedded (zero-config local inference with Qwen2.5-Coder GGUF)
- **BuildKit Integration**: buildkit-llb (LLB graph generation)
- **HTTP Client**: reqwest (async), genai (multi-provider)
- **CLI Framework**: clap (derive macros)
- **Error Handling**: anyhow, thiserror
- **Async Runtime**: tokio
- **Serialization**: serde, serde_json

## Wolfi-First Architecture

**BREAKING CHANGE:** aipack uses Wolfi packages exclusively for all container images.

### Core Principles

1. **No Base Image Configuration**: Base image is always `cgr.dev/chainguard/wolfi-base` (hardcoded in LLB generation)
2. **Package-Only Specification**: UniversalBuild schema contains only `packages` fields (build and runtime)
3. **Dynamic Version Discovery**: Wolfi package versions discovered from APKINDEX at runtime (24h cache)
4. **Version-Specific Packages**: All packages must be version-specific (e.g., `nodejs-22`, not `nodejs`)
5. **Package Validation**: Fuzzy matching and version-aware validation against Wolfi APKINDEX

### Wolfi Package Index

Located at `src/validation/wolfi_index.rs`, provides:

- **`fetch_apkindex()`**: Downloads APKINDEX.tar.gz from packages.wolfi.dev
- **`WolfiPackageIndex`**: In-memory index with fast lookups
  - `get_versions(prefix)`: Find all versions (e.g., "nodejs" → ["22", "20", "18"])
  - `get_latest_version(prefix)`: Get highest version
  - `has_package(name)`: Check exact package exists
  - `match_version(prefix, requested, available)`: Best version match
- **Binary Cache**: 30x performance improvement (70ms with warm cache)
- **Proper Semantic Versioning**: Handles multi-component versions correctly

### BuildSystem Integration

All `BuildSystem` implementations query `WolfiPackageIndex` for dynamic version discovery:

```rust
fn build_template(&self, wolfi_index: &WolfiPackageIndex, manifest_content: Option<&str>) -> BuildTemplate {
    // Parse version from manifest (e.g., package.json engines.node)
    let requested = parse_version(manifest_content);

    // Query available versions from Wolfi
    let available = wolfi_index.get_versions("nodejs");

    // Select best match or latest
    let version = wolfi_index.match_version("nodejs", &requested, &available)
        .or_else(|| wolfi_index.get_latest_version("nodejs"))
        .unwrap_or("nodejs-22");

    BuildTemplate {
        build_packages: vec![version.clone()],
        runtime_packages: vec![version],
        ...
    }
}
```

### Schema Changes

**UniversalBuild Schema (version 1.0):**
- **REMOVED**: `build.base` field (no base image configuration)
- **REMOVED**: `runtime.base` field (no base image configuration)
- **KEPT**: `build.packages` (Wolfi package names, version-specific)
- **KEPT**: `runtime.packages` (Wolfi package names, version-specific)
- **KEPT**: `version` field (remains "1.0" - removal is simplification, not breaking addition)

**BuildTemplate Struct:**
- **REMOVED**: `build_image` field
- **REMOVED**: `runtime_image` field
- **KEPT**: `build_packages` field
- **KEPT**: `runtime_packages` field

## BuildKit Frontend Architecture

aipack acts as a BuildKit frontend, generating Low-Level Build (LLB) definitions for image building.

### Frontend Workflow

1. **Detection** (`aipack detect`): Analyze repository → Generate UniversalBuild JSON
2. **LLB Generation** (`aipack frontend`): Read UniversalBuild → Generate LLB protobuf
3. **Build** (`buildctl build`): Execute LLB with BuildKit daemon

### LLB Generation

Located at `src/buildkit/llb.rs`, implements:

- **2-Stage Distroless Build**: Build stage + distroless final stage
- **Gitignore Filtering**: 99.995% context transfer reduction (1.5GB → 80-113KB)
- **Cache Mount Support**: Shared caches for build artifacts
- **Hardcoded Wolfi Base**: Always uses `cgr.dev/chainguard/wolfi-base:latest`

### Distroless Squashed Architecture

Final images use a 4-stage build with squashed runtime layer (no wolfi-base in history):

```
Stage 1 (Build):
  wolfi-base + build packages → build app → artifacts

Stage 2 (Runtime Prep):
  wolfi-base + runtime packages → remove apk

Stage 3 (Squash to Clean Base):
  glibc-dynamic (clean base, no apk) + copy runtime prep → squashed layer

Stage 4 (Final):
  squashed runtime + copy artifacts → final image
```

**Result:**
- Layer 1-5: `glibc-dynamic:latest` (clean base, ~11MB, no apk ever existed)
- Layer 6: Squashed runtime (~10MB, packages installed, apk removed)
- Layer 7: Application layer (~16MB, binary)
- Layer metadata: Clean descriptions (`: aipack <name> runtime`)

**Characteristics:**
- No `/sbin/apk` in filesystem (package manager removed)
- No `wolfi-base` in layer history (squashed to clean base)
- Truly distroless: apk never existed in any layer
- Optimized size: ~13MB total (aipack example)
- Production-ready by default (mandatory, no opt-out)

### Context Transfer Optimization

Located in `LLBBuilder::load_gitignore_patterns()`:

- Parses `.gitignore` file and extracts patterns
- Adds standard exclusions (`.git`, `.vscode`, `*.md`, `LICENSE`)
- Applies to `Source::local()` via `add_exclude_pattern()`
- Results in 99.995% reduction (1.54GB → 80-113KB for aipack itself)
- No filesystem state dependency - patterns embedded in LLB

### CLI Commands

**`aipack detect <repo>`**: Analyze repository and generate UniversalBuild JSON
- Runs 9-phase pipeline
- Validates packages against Wolfi APKINDEX
- Outputs JSON to stdout

**`aipack frontend --spec <file>`**: Generate LLB from UniversalBuild spec
- Reads UniversalBuild JSON
- Generates BuildKit LLB protobuf
- Outputs raw LLB to stdout for buildctl

### BuildKit Integration

#### Building Distroless Images

**Step 1: Generate LLB**
```bash
# Auto-detect and generate LLB (uses static detection by default)
AIPACK_DETECTION_MODE=static cargo run --release -- frontend > /tmp/llb.pb

# Or use full detection with LLM
cargo run --release -- frontend > /tmp/llb.pb
```

**Step 2: Start BuildKit**
```bash
# Using Docker container
docker run -d --rm --name buildkitd --privileged -p 127.0.0.1:1234:1234 \
  moby/buildkit:latest --addr tcp://0.0.0.0:1234
```

**Step 3: Build with buildctl**
```bash
# Build and export to Docker tar
cat /tmp/llb.pb | buildctl --addr tcp://127.0.0.1:1234 build \
  --local context=/path/to/repo \
  --output type=docker,name=localhost/myapp:latest > /tmp/myapp.tar

# Load into Docker
docker load < /tmp/myapp.tar

# Or build and pipe directly to docker load
cat /tmp/llb.pb | buildctl --addr tcp://127.0.0.1:1234 build \
  --local context=/path/to/repo \
  --output type=docker,name=localhost/myapp:latest | docker load
```

**Step 4: Verify**
```bash
# Check no apk in filesystem
docker run --rm localhost/myapp:latest test -f /sbin/apk && echo "FAIL" || echo "PASS"

# Check no wolfi-base in history
docker history localhost/myapp:latest | grep wolfi-base && echo "FAIL" || echo "PASS"

# View layer metadata
docker history localhost/myapp:latest --format "table {{.Size}}\t{{.CreatedBy}}"
```

#### With SBOM and Provenance

```bash
cat /tmp/llb.pb | buildctl --addr tcp://127.0.0.1:1234 build \
  --local context=/path/to/repo \
  --output type=docker,name=localhost/myapp:latest \
  --opt attest:sbom= \
  --opt attest:provenance=mode=max \
  | docker load
```

**Requirements:**
- BuildKit v0.11.0+ (Docker Desktop 4.17+, Docker Engine 23.0+)
- buildctl CLI tool
- Docker or Podman for loading images

## Project Structure

```
aipack/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── lib.rs               # Library root
│   ├── llm/                 # LLM client abstraction
│   │   ├── client.rs        # LLMClient trait
│   │   ├── genai.rs         # GenAI multi-provider client
│   │   ├── mock.rs          # MockLLMClient for testing
│   │   ├── recording.rs     # Request/response recording system
│   │   ├── selector.rs      # LLM client selection logic
│   │   └── embedded/        # Embedded local inference
│   ├── stack/               # Unified stack registry (languages, build systems, frameworks, orchestrators)
│   ├── languages/           # Language definitions (Rust, JS, Python, Java, Go, .NET, Ruby, PHP, C++, Kotlin, Elixir)
│   ├── build_systems/       # Build system definitions (Cargo, Maven, Gradle, npm, yarn, pnpm, pip, poetry, go mod, dotnet, composer)
│   ├── frameworks/          # Framework definitions (Spring Boot, Next.js, Django, Rails, etc.)
│   ├── fs/                  # FileSystem abstraction (real + mock)
│   ├── bootstrap/           # Pre-scan bootstrap
│   ├── progress/            # Progress reporting
│   ├── validation/          # Validation system
│   ├── extractors/          # Code-based extraction (port, env vars, health checks)
│   ├── heuristics/          # Heuristic logging
│   ├── pipeline/            # Analysis pipeline
│   │   ├── orchestrator.rs  # PipelineOrchestrator (9-phase pipeline)
│   │   └── phases/          # Pipeline phases (scan, classify, structure, dependencies, build_order, runtime, build, entrypoint, native_deps, port, env_vars, health, cache, root_cache, assemble)
│   ├── detection/           # Detection service (public API)
│   ├── output/              # Output formatting (JSON schema)
│   ├── cli/                 # Command-line interface
│   └── config.rs            # Configuration management
├── tests/
│   ├── e2e.rs               # End-to-end tests with fixtures
│   ├── fixtures/            # Test fixture repositories
│   │   ├── single-language/ # Single build system projects
│   │   ├── monorepo/        # Monorepo/workspace projects
│   │   ├── edge-cases/      # Edge cases and unusual configurations
│   │   └── expected/        # Expected JSON outputs
│   └── recordings/          # LLM request/response recordings
├── Cargo.toml               # Project manifest
├── PRD.md                   # Product requirements
├── CHANGELOG.md             # Version history
└── README.md                # User documentation
```

## Phase-Based Pipeline Architecture

aipack uses a 9-phase sequential pipeline where code orchestrates the workflow and LLMs are used only for unknowns.

### Pipeline Phases

```
1. Scan          → Pre-scan repository using BootstrapScanner
2. Classify      → Identify if monorepo or single project (LLM)
3. Structure     → Detect project structure and layout (LLM)
4. Dependencies  → Parse dependency graphs (deterministic + LLM fallback)
5. Build Order   → Topological sort of build dependencies (deterministic)
6. Service Analysis (per service):
   6a. Runtime   → Detect language/framework runtime (LLM)
   6b. Build     → Extract build commands (LLM)
   6c. Entrypoint→ Find application entrypoint (LLM)
   6d. Native Deps→ Identify system packages needed (LLM)
   6e. Port      → Discover exposed ports (deterministic + LLM)
   6f. Env Vars  → Extract environment variables (deterministic + LLM)
   6g. Health    → Find health check endpoints (deterministic + LLM)
7. Cache         → Map cache directories by build system (deterministic)
8. Root Cache    → Detect monorepo root cache (deterministic)
9. Assemble      → Combine results into UniversalBuild (deterministic)
```

### Key Design Principles

- **Code-Driven**: Pipeline orchestration is deterministic, not LLM-controlled
- **Minimal Context**: Each LLM prompt uses <500 tokens (vs 10k-50k in tool-based approach)
- **Deterministic First**: Use parsers for known formats, LLM only for unknowns
- **Sequential Execution**: Simple linear processing (no async complexity)
- **Heuristic Logging**: All LLM calls logged for future optimization

### Benefits

- **85-95% token reduction**: From 10k-50k to 1k-6k tokens per detection
- **Supports smallest models**: 8k context sufficient (enables 0.5B-1.5B models)
- **Predictable cost**: Fixed max LLM calls (7-9 prompts vs unbounded iteration)
- **Debuggable**: Each phase has clear input/output
- **Deterministic cache detection**: Build system knowledge, not LLM guessing

## Using the Detection Service

```rust
use aipack::detection::DetectionService;
use aipack::llm::selector::select_llm_client;
use std::path::PathBuf;

// Auto-select LLM client (tries Ollama, falls back to embedded)
let client = select_llm_client().await?;

// Create detection service
let service = DetectionService::new(client)?;

// Detect build system (returns Vec<UniversalBuild>)
let results = service.detect(PathBuf::from("/path/to/repo")).await?;
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

The `select_llm_client()` function automatically selects the best available LLM client:

1. **Environment variables** - If `AIPACK_PROVIDER` is set, use that provider
2. **Ollama** - Try connecting to Ollama (localhost:11434)
3. **Embedded** - Fall back to embedded local inference (zero-config)

Running with different providers:
```bash
# Auto-select (tries Ollama, falls back to embedded)
cargo run -- detect /path/to/repo

# Force specific provider
AIPACK_PROVIDER=ollama cargo run -- detect /path/to/repo
AIPACK_PROVIDER=claude ANTHROPIC_API_KEY=sk-... cargo run -- detect /path/to/repo
AIPACK_PROVIDER=embedded cargo run -- detect /path/to/repo
```

## Configuration & Environment

### Aipack Configuration Environment Variables

```bash
# Provider selection (defaults to "ollama")
AIPACK_PROVIDER=ollama             # "ollama", "openai", "claude", "gemini", "grok", or "groq"

# Model configuration
AIPACK_MODEL=qwen2.5-coder:7b      # Model name for selected provider

# Request configuration
AIPACK_REQUEST_TIMEOUT=60          # Request timeout in seconds
AIPACK_MAX_CONTEXT_SIZE=512000     # Maximum context size in tokens
AIPACK_MAX_TOKENS=8192             # Max tokens per LLM response (default: 8192, min: 512, max: 128000)

# Logging
RUST_LOG=aipack=debug,info         # Structured logging

# Embedded model configuration
AIPACK_MODEL_SIZE=7B               # Explicit model size: "0.5B", "1.5B", "3B", or "7B" (overrides auto-selection)

# Detection mode control
AIPACK_DETECTION_MODE=full         # "full" (default), "static", or "llm"
                                   # full: LLM + static analysis (deterministic first, LLM fallback)
                                   # static: Static analysis only, no LLM calls (fast, deterministic)
                                   # llm: LLM-only detection (for testing LLM path specifically)
```

### Provider-Specific Environment Variables

These are managed by the `genai` crate:

```bash
# Ollama (local inference)
OLLAMA_HOST=http://localhost:11434   # Optional, defaults to localhost:11434

# OpenAI
OPENAI_API_KEY=sk-proj-...           # Required for OpenAI

# Anthropic Claude
ANTHROPIC_API_KEY=sk-ant-api03-...   # Required for Claude

# Google Gemini
GOOGLE_API_KEY=AIza...               # Required for Gemini

# xAI Grok
XAI_API_KEY=xai-...                  # Required for Grok

# Groq
GROQ_API_KEY=gsk_...                 # Required for Groq
```

### Embedded Model Selection

When using the embedded backend, aipack runs local inference using Qwen2.5-Coder models in GGUF format (Q4 quantized).

#### Automatic Selection (Default)
By default, aipack auto-selects the largest model that fits in available RAM (reserves 25% or 2GB minimum for system):
```bash
./aipack detect .
```

#### Explicit Model Size Selection
Override auto-selection with `AIPACK_MODEL_SIZE`:
```bash
AIPACK_MODEL_SIZE=0.5B ./aipack detect .   # Smallest (requires ~1GB RAM)
AIPACK_MODEL_SIZE=1.5B ./aipack detect .   # Small (requires ~2.5GB RAM)
AIPACK_MODEL_SIZE=3B ./aipack detect .     # Medium (requires ~4GB RAM)
AIPACK_MODEL_SIZE=7B ./aipack detect .     # Largest (requires ~5.5GB RAM)
```

All models use GGUF format with Q4_K_M quantization and support tool calling.

### LLM Self-Reasoning Loop Prevention

aipack includes safeguards to prevent LLMs from getting stuck in self-reasoning loops:

1. **Token Limits**: `AIPACK_MAX_TOKENS` (default: 8192) prevents runaway generation
2. **Stop Sequences**: Automatically applied to catch repetitive patterns: `</thinking>`, `In summary:`, `To reiterate:`, `Let me repeat:`
3. **Per-Call Timeouts**: Each LLM API call enforces the configured timeout
4. **Concise Prompt**: System prompt discourages verbose reasoning

## Architecture & Design Patterns

### LLMClient Trait
```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
}
```

All LLM integrations implement this trait, providing pluggable backends.

### StackRegistry

The `StackRegistry` is a unified registry for all technology stack components with strongly-typed identifiers:

```rust
use aipack::stack::{StackRegistry, LanguageId, BuildSystemId, FrameworkId};

let registry = StackRegistry::with_defaults();

let stack = registry.detect_stack(manifest_path, content)?;

let build_system = registry.get_build_system(BuildSystemId::Cargo)?;
let language = registry.get_language(LanguageId::Rust)?;
let framework = registry.get_framework(FrameworkId::ActixWeb)?;
```

**Key Features:**
- **Type-safe IDs**: All technology types use strongly-typed enums (LanguageId, BuildSystemId, FrameworkId, OrchestratorId)
- **Auto-discovery**: Components automatically register their IDs via trait methods
- **Unified detection**: Single `detect_stack()` call returns complete technology stack
- **Relationship validation**: Built-in compatibility checking between languages, build systems, and frameworks

**Supported Technologies:**
- **Languages** (13): Rust, Java, Kotlin, JavaScript, TypeScript, Python, Go, C#, F#, Ruby, PHP, C++, Elixir
- **Build Systems** (16): Cargo, Maven, Gradle, npm, yarn, pnpm, Bun, pip, poetry, pipenv, go mod, dotnet, composer, bundler, CMake, mix
- **Frameworks** (20): Spring Boot, Quarkus, Micronaut, Ktor, Express, Next.js, Nest.js, Fastify, Django, Flask, FastAPI, Rails, Sinatra, Actix-web, Axum, Gin, Echo, ASP.NET Core, Laravel, Phoenix
- **Orchestrators** (3): Turbo, Nx, Lerna

### AnalysisContext and Phase Traits

The pipeline uses a context-based architecture where `AnalysisContext` accumulates state as phases execute:

```rust
use aipack::pipeline::{AnalysisContext, WorkflowPhase, ServicePhase, ServiceContext};

// AnalysisContext holds all pipeline state
pub struct AnalysisContext {
    pub repo_path: PathBuf,
    pub llm_client: Arc<dyn LLMClient>,
    pub stack_registry: Arc<StackRegistry>,
    pub progress_handler: Option<LoggingHandler>,
    pub heuristic_logger: Arc<HeuristicLogger>,

    // Phase results (populated as phases execute)
    pub scan: Option<ScanResult>,
    pub classify: Option<ClassifyResult>,
    pub structure: Option<StructureResult>,
    pub dependencies: Option<DependencyResult>,
    pub build_order: Option<BuildOrderResult>,
    pub root_cache: Option<RootCacheInfo>,
}
```

**WorkflowPhase Trait** - Repository-level phases:
```rust
#[async_trait]
pub trait WorkflowPhase: Send + Sync {
    async fn execute(&self, context: &mut AnalysisContext) -> Result<()>;
    fn name(&self) -> &'static str;
}
```

Repository phases (scan, classify, structure, dependencies, build_order, root_cache) implement `WorkflowPhase` and write their results to the context.

**ServicePhase Trait** - Service-level phases:
```rust
#[async_trait]
pub trait ServicePhase: Send + Sync {
    async fn execute(&self, context: &ServiceContext<'_>) -> Result<ServicePhaseResult>;
    fn name(&self) -> &'static str;
}
```

Service phases (runtime, build, entrypoint, native_deps, port, env_vars, health) implement `ServicePhase` and return `ServicePhaseResult` variants. They operate on `ServiceContext` which provides access to the full `AnalysisContext`:

```rust
pub struct ServiceContext<'a> {
    pub service: &'a Service,
    pub analysis_context: &'a AnalysisContext,
    pub runtime: Option<&'a RuntimeInfo>,
}

// Helper methods for clean access to common fields
impl<'a> ServiceContext<'a> {
    pub fn repo_path(&self) -> &Path { ... }
    pub fn scan(&self) -> &ScanResult { ... }
    pub fn dependencies(&self) -> &DependencyResult { ... }
    pub fn llm_client(&self) -> &dyn LLMClient { ... }
    pub fn stack_registry(&self) -> &Arc<StackRegistry> { ... }
    pub fn heuristic_logger(&self) -> &Arc<HeuristicLogger> { ... }
}
```

**Key Benefits:**
- Single source of truth for pipeline state
- Type-safe phase dependencies (panic if prerequisite missing)
- Clear separation between repository and service analysis
- Service phases have access to full context for future extensibility
- Helper methods keep the API clean and explicit
- Easier testing with mock contexts
- Future-proof: when service results are flattened into global context, phases can access them

### UniversalBuild Output
Multi-stage container build specification containing:
- **Metadata**: project name, language, build system, confidence, reasoning
- **Build Stage**: base image, packages, environment variables, build commands, context, cache paths, artifacts
- **Runtime Stage**: base image, packages, environment variables, copy specifications, command, ports, healthcheck
- Schema version with validation

**Note**: For monorepos, `DetectionService.detect()` returns `Vec<UniversalBuild>` with one entry per runnable application.

## Test Fixtures

Test fixtures validate build system detection across different languages and project structures. Located in `tests/fixtures/`:

- **single-language/**: Single build system projects (rust-cargo, node-npm, python-pip, java-maven, go-mod, dotnet-csproj, etc.)
- **monorepo/**: Monorepo/workspace projects (npm-workspaces, turborepo, cargo-workspace, gradle-multiproject, polyglot)
- **edge-cases/**: Edge cases (empty-repo, no-manifest, multiple-manifests, nested-projects)
- **expected/**: Expected JSON outputs for validation

Fixtures follow these principles:
1. **Minimal**: Only essential files for detection
2. **Representative**: Real-world project structures
3. **Working**: Can actually build/run with the specified tools
4. **Complete**: Include source code, manifests, and dependencies

## LLM Recording System

The recording system captures LLM request/response pairs for deterministic testing without requiring live LLM access. This enables CI/CD testing without API keys, regression testing against known-good responses, and faster tests.

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

Recordings are stored in `tests/recordings/` with filenames based on request content hash.

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

## Dual-Mode Testing

The e2e test suite supports three detection modes to validate both LLM and static analysis code paths:

### Detection Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `full` | LLM + static analysis (default) | Normal operation - deterministic first, LLM fallback |
| `static` | Static analysis only, no LLM | Fast CI tests, validate deterministic detection |
| `llm` | LLM-only detection | Test LLM code paths specifically |

### Test Organization

Each fixture has three test variants:
- `test_*_detection()` - Full mode (original tests, default behavior)
- `test_*_llm()` - LLM mode test
- `test_*_static()` - Static-only mode test

Example:
```rust
#[test]
fn test_rust_cargo_detection() { ... }  // Full mode (default)

#[test]
fn test_rust_cargo_llm() { ... }        // LLM mode

#[test]
fn test_rust_cargo_static() { ... }     // Static mode (fast, no LLM)
```

### Running Tests by Mode

```bash
# Run all tests (default mode)
cargo test --test e2e

# Run only static mode tests (fast, deterministic, no LLM backend needed)
cargo test --test e2e static

# Run only LLM mode tests
cargo test --test e2e llm

# Run specific static test
cargo test --test e2e test_rust_cargo_static
```

### Benefits

1. **Fast CI**: Static mode tests run without LLM backend (< 10 seconds for all fixtures)
2. **Complete Coverage**: Both LLM and static paths validated via CLI
3. **Deterministic**: Static mode is fully deterministic, no LLM variance
4. **True E2e**: All tests spawn the full binary, validating CLI integration

### When to Use Each Mode

- **Full mode** (`AIPACK_DETECTION_MODE=full`): Default for normal detection
- **Static mode** (`AIPACK_DETECTION_MODE=static`): Fast CI tests, validate parsers work correctly
- **LLM mode** (`AIPACK_DETECTION_MODE=llm`): Test LLM prompts and response handling specifically
