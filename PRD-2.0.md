# aipack - Product Requirements Document 2.0

## Executive Summary

**aipack** is an AI-powered build system intelligence tool that uses LLM reasoning with function-calling capabilities to analyze repositories and generate accurate build commands. Unlike traditional pattern-matching tools, aipack leverages iterative exploration where the LLM dynamically decides what files to inspect, enabling it to handle any repository structure—from simple single-app projects to complex monorepos—without hardcoded detection rules.

## Evolution from 1.0

### What Changed
The original aipack concept focused on passing full repository context to an LLM for analysis. Through architectural exploration, we evolved to a **tool-based detection system** where:

- **LLM drives exploration** rather than consuming pre-gathered context
- **Function calling** enables iterative file system inspection
- **No context window limitations** since only requested files are read
- **Reasoning-first approach** eliminates brittle pattern matching
- **Automatic monorepo detection** handles any repository structure

### Why This Matters
Traditional build detection tools fail on:
- Unconventional project structures
- Mixed-language monorepos
- Custom build configurations
- Novel frameworks and tooling

By giving the LLM **agency** through tools, aipack can reason about any repository structure and adapt its exploration strategy dynamically.

## Vision

Build a universal repository intelligence system that understands build systems through reasoning rather than rules, making it trivial for developers and platforms to work with unfamiliar codebases.

## Core Architecture

### Tool-Based Detection System

Instead of traditional static analysis, aipack uses an **agent-based approach**:

```
Repository
    ↓
Root Agent (with tools)
    ├─ list_dir(path) → explore structure
    ├─ read_file(path) → inspect manifests
    ├─ search(pattern) → find specific files
    ├─ get_file_tree() → understand layout
    └─ grep_content(regex) → search content
    ↓
Detection Decision
    ├─ Single app → analyze directly
    └─ Monorepo → spawn sub-agents per project
    ↓
Build Definition(s)
```

### Available Tools

The LLM has access to 6 filesystem tools:

| Tool               | Purpose                                    | Example                          |
|--------------------|--------------------------------------------|----------------------------------|
| `list_files`       | List directory contents with optional glob | Find all `package.json` files    |
| `read_file`        | Read file contents (size-limited)          | Read `Cargo.toml`                |
| `search_files`     | Search by filename pattern                 | Find `*.gradle` files            |
| `get_file_tree`    | Tree view of directory                     | Understand layout                |
| `grep_content`     | Search file contents with regex            | Find `"scripts"` in package.json |
| `submit_detection` | Submit final result                        | Return build definition          |

### Key Principles

1. **Reasoning Over Rules**: LLM decides exploration strategy based on what it discovers
2. **Iterative Refinement**: Can explore deeper when ambiguous
3. **Bounded Exploration**: Safety limits on iterations, file sizes, and scope
4. **Unified Detection**: Same logic handles single apps and monorepos
5. **Provider Agnostic**: Works with any LLM supporting function calling

## Multi-Level Reasoning & Large Repository Optimization

### Hierarchical Exploration Strategy

aipack uses a **progressive depth approach** to analyze repositories efficiently, especially critical for large monorepos with thousands of files.

#### Level 0: Structural Survey (Quick Pass)
**Goal**: Understand high-level organization without reading files
- **Tool**: `get_file_tree` with depth=2
- **Strategy**: Get bird's-eye view of repository structure
- **Decision Point**: Single app vs monorepo classification
- **Token Cost**: Minimal (~100-500 tokens)

```
Example insight:
/repo
  ├── apps/          ← Multiple apps indicator
  ├── services/      ← Monorepo pattern
  ├── libs/          ← Shared code
  └── package.json   ← Workspace orchestrator?
```

#### Level 1: Manifest Discovery (Targeted Scan)
**Goal**: Locate all build-relevant files without reading content
- **Tool**: `search_files` with patterns: `["**/package.json", "**/Cargo.toml", "**/pom.xml", "**/go.mod", "**/requirements.txt", "**/build.gradle*"]`
- **Strategy**: Find project roots through manifest files
- **Optimization**: Exclude patterns (`node_modules/`, `.git/`, `vendor/`)
- **Token Cost**: Low (~200-1000 tokens depending on repo size)

**Smart Filtering**:
```yaml
# Only search in likely project directories
include_patterns:
  - "apps/**"
  - "services/**"
  - "packages/**"
  - "src/**"

# Skip dependency directories
exclude_patterns:
  - "**/node_modules/**"
  - "**/vendor/**"
  - "**/.git/**"
  - "**/target/**"
  - "**/dist/**"
  - "**/build/**"
```

#### Level 2: Manifest Inspection (Selective Reading)
**Goal**: Read only the files needed to determine build systems
- **Tool**: `read_file` on discovered manifests
- **Strategy**: Prioritize root-level manifests, then workspace definitions
- **Optimization**:
  - Read only first 2KB of large files (enough for dependencies, scripts)
  - Use `grep_content` to check specific keys before full read
  - Cache manifest contents to avoid re-reading

**Progressive Reading Example**:
```
1. Check if package.json has "workspaces" field → monorepo
2. If yes: read workspace root package.json only
3. For each workspace: read manifest on-demand when analyzing that project
```

#### Level 3: Context Refinement (Deep Dive)
**Goal**: Gather additional context for ambiguous cases
- **Tools**: `read_file` on README, `grep_content` for specific patterns
- **Strategy**: Only invoked when confidence < 0.7
- **Examples**:
  - Read README to understand custom build processes
  - Check for Makefile or build scripts
  - Inspect .github/workflows/ for CI hints

### Optimization Techniques for Large Repositories

#### 1. Lazy Evaluation
**Principle**: Only fetch data when decision depends on it

```python
# Bad: Read everything upfront
all_files = read_all_manifests()
classify(all_files)

# Good: Progressive refinement
tree = get_file_tree()
if looks_like_monorepo(tree):
    manifests = search_files("**/package.json")
    for manifest in manifests:
        # Read only if needed for classification
        if not is_dependency_folder(manifest):
            content = read_file(manifest)
```

#### 2. External Memory
**Problem**: Context window limitations with large repositories and monorepos

**Solution**: Use external memory to store exploration state and findings

**Architecture**:
```
┌─────────────────┐
│   LLM Agent     │
│  (Stateless)    │
└────────┬────────┘
         │
         ↓
┌─────────────────────────┐
│   External Memory       │
│  ┌──────────────────┐   │
│  │ Exploration Log  │   │  ← Tool calls & results
│  ├──────────────────┤   │
│  │ Findings Summary │   │  ← Detected projects
│  ├──────────────────┤   │
│  │ Decisions Made   │   │  ← Classifications
│  ├──────────────────┤   │
│  │ Next Actions     │   │  ← Planned steps
│  └──────────────────┘   │
└─────────────────────────┘
```

**Memory Structure**:
```json
{
  "exploration_log": [
    {
      "step": 1,
      "tool": "get_file_tree",
      "result_summary": "Found apps/, services/, libs/ directories",
      "decision": "Likely monorepo structure"
    },
    {
      "step": 2,
      "tool": "search_files",
      "pattern": "**/package.json",
      "found": 23,
      "decision": "Node.js workspace detected"
    }
  ],
  "discovered_projects": [
    {
      "path": "apps/web",
      "language": "TypeScript",
      "confidence": 0.9
    }
  ],
  "workspace_info": {
    "type": "pnpm",
    "root": "/",
    "patterns": ["apps/*", "packages/*"]
  },
  "next_steps": [
    "Analyze apps/web/package.json",
    "Check for shared dependencies"
  ]
}
```

**Benefits**:
- **Unlimited Context**: No token limit on stored information
- **Structured State**: Organized exploration history
- **Resume Capability**: Can pause and resume analysis
- **Multi-Agent Support**: Shared memory for sub-agents analyzing different projects
- **Audit Trail**: Complete record of reasoning process

**Implementation**:
- Store in-memory during analysis
- Optionally persist to disk for long-running analyses
- Query interface for LLM: "What projects have we discovered so far?"
- Update interface: "Add new finding: detected Rust workspace"

#### 3. Smart File Size Handling
**Principle**: Avoid reading massive files entirely

**Strategies**:
- **Large manifests** (>100KB): Read first 5KB only (usually sufficient)
- **README files** (>50KB): Read first 10KB or search for specific sections
- **Binary detection**: Skip .lock files, images, compiled artifacts
- **Truncation markers**: Add `[truncated at 5KB]` to inform LLM

**Example**:
```python
def smart_read(path, max_size=5120):
    size = get_file_size(path)
    if size > max_size:
        content = read_bytes(path, max_size)
        return f"{content}\n\n[File truncated: {size} bytes total, showing first {max_size} bytes]"
    return read_file(path)
```

#### 4. Early Termination Conditions
**Principle**: Stop exploration when confidence is sufficient

**Confidence Scoring**:
```yaml
confidence_levels:
  high: 0.9-1.0    # Strong evidence, stop exploring
  medium: 0.7-0.9  # Validate with 1-2 more checks
  low: 0.0-0.7     # Continue exploring or flag as uncertain

stop_conditions:
  - confidence >= 0.9
  - iterations >= max_iterations (10)
  - timeout reached (30s)
  - no more useful tools to call
```

**Example Decision Tree**:
```
Found Cargo.toml at root + src/main.rs?
  → Confidence: 0.95 → STOP (clear Rust project)

Found package.json + tsconfig.json?
  → Confidence: 0.85 → Read package.json scripts
  → If has "build": "tsc"
    → Confidence: 0.95 → STOP (TypeScript project)
```

#### 5. Workspace-Aware Analysis
**Problem**: Monorepo tools (pnpm, Yarn workspaces, Cargo workspaces, Bazel) have special semantics

**Strategy**: Detect workspace orchestrator first
```python
# Priority order
workspace_indicators = [
    ("pnpm-workspace.yaml", "pnpm_workspaces"),
    ("lerna.json", "lerna"),
    ("nx.json", "nx"),
    ("Cargo.toml[workspace]", "cargo_workspace"),
    ("go.work", "go_workspaces"),
    ("BUILD.bazel", "bazel"),
]

# If detected, use workspace-specific logic
if workspace_type == "pnpm_workspaces":
    # Read workspace config to get package patterns
    config = read_file("pnpm-workspace.yaml")
    patterns = extract_workspace_patterns(config)

    # Search only those patterns (not entire repo)
    projects = search_files(patterns)
```

**Workspace-Specific Optimizations**:
- **pnpm/Yarn**: Parse `workspaces` field for exact paths
- **Cargo**: Parse `[workspace] members` for project list
- **Bazel**: Use `BUILD` files as authoritative source
- **Go**: Use `go.work` to list modules

## Use Cases

### 1. Single Application Analysis
```bash
aipack detect /path/to/rust-project
# LLM explores: finds Cargo.toml → reads it → detects Rust → returns build commands
```

### 2. Monorepo Discovery
```bash
aipack detect /path/to/monorepo
# Root agent: lists directories → finds apps/, services/ → identifies manifests
#            → spawns sub-agents → each analyzes its project
```

### 3. Ambiguous Structure
```bash
aipack detect /path/to/complex-repo
# LLM: lists root → sees multiple tools → reads README → checks scripts
#     → reasons about primary vs auxiliary → makes decision
```

### 4. Nested Monorepos
```bash
aipack detect /path/to/workspace
# Recursively explores: can handle services/backend/api (nested structure)
```

## Technical Specification

### LLM Provider Support

**Primary: GenAI Multi-Provider Client**

Unified interface supporting:
- **Ollama** (local inference) - Qwen, Mistral, etc.
- **Anthropic Claude** - Claude Sonnet 4.5+
- **OpenAI** - GPT-4, GPT-3.5 Turbo
- **Google Gemini** - Gemini Pro
- **xAI Grok** - Grok-1
- **Groq** - Mixtral, LLaMA

All providers accessed through consistent `LLMBackend` trait.

**Fallback: Embedded LLM (Phi-3 Mini)**

When GenAI providers are unavailable or not configured, aipack automatically falls back to an embedded LLM:

- **Model**: Phi-3 Mini (3.8B parameters)
- **Auto-Download**: Model weights downloaded automatically on first use
- **Storage**: Cached in `~/.aipack/models/` (~2GB for 4-bit quantized)
- **Performance**:
  - CPU inference: ~10-20 tokens/sec on modern CPUs
  - Memory: ~2-3GB RAM required
  - No GPU required
- **License**: MIT (commercial use allowed)
- **Benefits**:
  - Zero configuration required
  - Works offline
  - No API costs
  - Privacy-preserving (no data leaves machine)

This ensures aipack works out-of-the-box without external dependencies, while still supporting higher-quality models when available.

### Configuration

```yaml
# Environment variables
AIPACK_PROVIDER=ollama              # Provider selection
AIPACK_MODEL=qwen2.5-coder:7b       # Model name
AIPACK_MAX_TOOL_ITERATIONS=10       # Max exploration depth (safety)
AIPACK_TOOL_TIMEOUT=30              # Tool execution timeout (seconds)
AIPACK_MAX_FILE_SIZE=1048576        # Max file read size (1MB)

# Provider-specific (handled by genai crate)
OLLAMA_HOST=http://localhost:11434
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
GOOGLE_API_KEY=...
```

### Detection Flow

1. **Initialization**: Configure LLM backend and tool registry
2. **Root Analysis**:
   - LLM receives system prompt explaining available tools
   - Starts exploration from repository root
   - Iteratively calls tools based on reasoning
3. **Classification Decision**:
   - Single app: proceeds to build definition
   - Monorepo: identifies sub-project paths
4. **Sub-Project Analysis** (if monorepo):
   - Spawn sub-agents for each project
   - Each explores its scope independently
   - Parallel execution with concurrency limits
5. **Aggregation**: Combine results into unified output

### Safety & Performance

**Safety Limits**:
- Max iterations per analysis: 10 (configurable, hard cap: 50)
- Tool execution timeout: 30s (configurable, hard cap: 300s)
- Max file size: 1MB (configurable, hard cap: 10MB)
- No binary file reading
- Sandboxed file access

**Performance**:
- Parallel sub-project analysis for monorepos
- Tool output caching (avoid re-reading same files)
- Streaming for large file trees
- Smart context management (only send what's needed)

### Output Format

aipack outputs a `universalbuild.yaml` format that describes how to build and run applications in a language-agnostic way:

**Single Project Output:**
```yaml
version: "1"

# --- BUILD STAGE ---
build:
  # Base image providing compiler or runtime
  base: golang:1.23-alpine

  # Optional system packages to install
  packages:
    - git
    - build-base

  # Environment variables during build
  env:
    GO111MODULE: "on"

  # Command to build the app
  command: ["go", "build", "-o", "bin/app", "."]

  # Paths relative to context to include in the build
  context:
    - go.mod
    - go.sum
    - cmd/
    - pkg/
    - internal/

  # Cache directories to persist between builds
  cache:
    - /go/pkg/mod
    - /root/.cache/go-build

  # Files to copy from build stage into runtime image
  artifacts:
    - bin/app

# --- RUNTIME STAGE ---
runtime:
  # Base image for final container
  base: alpine:3.20

  # Additional runtime packages
  packages:
    - ca-certificates

  # Environment variables in the runtime container
  env:
    APP_ENV: "production"

  # Copy artifacts from build stage to this path
  copy:
    - from: build
      src: bin/app
      dest: /usr/local/bin/app

  # Command to start the app
  command: ["/usr/local/bin/app"]

  # Ports exposed by the app
  ports:
    - 8080

  # Optional healthcheck
  healthcheck:
    test: ["CMD", "wget", "-q", "--spider", "http://localhost:8080/health"]
    interval: 30s
    timeout: 5s
    retries: 3
```

**Monorepo Output:**
For monorepos, multiple `universalbuild.yaml` files are generated (one per project):

```
/path/to/repo/
  ├── services/api/universalbuild.yaml
  ├── services/worker/universalbuild.yaml
  └── apps/frontend/universalbuild.yaml
```

Each file follows the same schema, tailored to that specific project's build requirements.

## Model Selection Strategy

### Recommended Models

**For Local Inference** (privacy, cost, offline):
- **Qwen 2.5 Coder 7B**: Best cost/performance for code reasoning
- **Mistral 7B Instruct**: Strong general reasoning
- **Phi-3 Mini (3.8B)**: Minimal resource footprint
- **Mixtral 8x7B**: Maximum accuracy (higher cost)

**For Cloud API** (always available, scalable):
- **Claude Sonnet 4.5**: Best reasoning quality
- **GPT-4 Turbo**: Strong general purpose
- **Gemini Pro**: Cost-effective alternative

### Selection Criteria

| Criterion          | Phi-3 Mini    | Qwen 2.5 7B   | Mistral 7B    | Claude Sonnet   |
|--------------------|---------------|---------------|---------------|-----------------|
| Parameters         | 3.8B          | 7B            | 7B            | Unknown (large) |
| VRAM (4-bit quant) | ~2GB          | ~4GB          | ~4GB          | N/A (API)       |
| CPU Speed (approx) | ~10 tokens/s  | ~5 tokens/s   | ~5 tokens/s   | ~50 tokens/s    |
| Code Reasoning     | Good          | Excellent     | Strong        | Best            |
| Cost (local)       | Hardware only | Hardware only | Hardware only | $3-15/1M tokens |
| License            | MIT           | Apache 2.0    | Apache 2.0    | Commercial API  |

### Quantization Strategy

**4-bit Quantization** (llama.cpp, GGUF):
- Phi-3 Mini: ~2GB RAM, acceptable quality loss
- Qwen/Mistral 7B: ~4GB RAM, minimal quality loss
- Runs on CPU (10-20 tokens/sec on modern CPU)
- Best for: development, homelab, edge deployment

**8-bit Quantization**:
- Better quality, ~2x memory vs 4-bit
- Best for: GPU with limited VRAM

**Full Precision**:
- Maximum quality
- Requires 16GB+ VRAM for 7B models
- Best for: production with dedicated hardware

## Comparison: Before vs After

| Aspect               | PRD 1.0 (Context-Based) | PRD 2.0 (Tool-Based)                 |
|----------------------|-------------------------|--------------------------------------|
| **Approach**         | Pass full repo context  | Iterative exploration                |
| **Context Limit**    | Limited by token window | Unlimited (only reads what's needed) |
| **Monorepo Support** | Requires pre-scan       | Automatic discovery                  |
| **Extensibility**    | Fixed prompt patterns   | LLM chooses strategy                 |
| **Scale**            | Fails on large repos    | Scales to any size                   |
| **Reasoning**        | Single-shot inference   | Multi-step reasoning                 |
| **Accuracy**         | Good for simple cases   | Excellent for complex cases          |

## Success Metrics

### Detection Quality
- **Accuracy**: >95% correct build command for standard projects
- **Monorepo Discovery**: 100% detection of all buildable sub-projects
- **Confidence Calibration**: High-confidence results >90% accurate

### Performance
- **Latency**:
  - Single app: <5s (local), <10s (cloud API)
  - Monorepo: <30s for <10 projects
- **Tool Efficiency**: <10 tool calls for 80% of repositories
- **Resource Usage**: <2GB RAM for inference (quantized local models)

### Reliability
- **Failure Rate**: <5% unhandled errors
- **Timeout Handling**: Graceful degradation on complex repos
- **Provider Fallback**: Auto-retry with different model if supported

### Adoption
- **Community Use**: 1000+ installations within 6 months
- **Open Source Contributions**: 20+ external contributors
- **Production Usage**: Adoption by CI/CD platforms and developer tools

## Security & Privacy

### Security Considerations
- **Sandboxed Execution**: Tools operate within repository boundaries
- **No Credential Leakage**: Never read .env, secrets files
- **Size Limits**: Prevent DoS via large file reads
- **Timeout Protection**: Hard caps on execution time
- **Binary File Filtering**: Avoid processing non-text files

### Privacy Modes
- **Local-Only**: All inference via Ollama (no data leaves machine)
- **Cloud API**: Repository metadata sent to provider (configurable)
- **Hybrid**: Local for discovery, cloud for complex reasoning

### Compliance
- **No Training**: Explicitly opt-out of model training with API providers
- **Data Retention**: No persistent storage of repository data
- **Audit Logging**: Optional tool call logging for debugging

## Cost Analysis

### Self-Hosted (Quantized 7B Model)
**One-Time Costs**:
- GPU (RTX 3060 12GB): $300-400
- Or: CPU inference (slower but no GPU needed)

**Ongoing Costs**:
- Power: ~$5-10/month (GPU running 24/7)
- Maintenance: Negligible

**Per-Analysis Cost**: $0 (amortized over hardware)

**Best For**: High volume (>1000 analyses/month), privacy requirements

### Cloud API (e.g., OpenAI, Claude)
**Pricing Example (Claude Sonnet 4.5)**:
- Input: $3/1M tokens
- Output: $15/1M tokens
- Typical analysis: ~2K input + 500 output = ~$0.01

**Monthly Cost Estimate**:
- 100 analyses: ~$1
- 1,000 analyses: ~$10
- 10,000 analyses: ~$100

**Best For**: Low/medium volume, no hardware investment, maximum quality

### Hybrid Approach
- Use local Qwen 7B for 90% of cases (straightforward repos)
- Fallback to Claude for complex monorepos or low-confidence results
- Estimated cost: ~$20-30/month for 5,000 analyses

## Open Questions & Future Research

1. **Optimal Model Size**: Is 3.8B (Phi-3) sufficient or 7B necessary?
2. **Context vs Tools Trade-off**: When to pass more context vs more tool calls?
3. **Multi-Agent Coordination**: How to optimize parallel sub-project analysis?
4. **Tool Design**: Are 6 tools sufficient or should we add more?
5. **Confidence Calibration**: How to reliably predict detection quality?
6. **Build Execution Safety**: How to safely run builds for validation?
7. **Template Library**: Should we maintain build pattern templates for common stacks?
8. **Cross-Language Patterns**: Can we detect polyglot projects better?

## Technical Debt & Risks

### Current Limitations
- **No Build Validation**: Detection untested against actual builds
- **Limited Template Support**: No pre-built patterns for acceleration
- **Single-Threaded Tool Execution**: Each tool call is sequential
- **No Caching**: Repeated analyses don't leverage prior results
- **Basic Error Handling**: Limited recovery from LLM failures

### Risk Mitigation
- **Model Availability**: Support multiple providers (no vendor lock-in)
- **Quality Variance**: Implement confidence scoring and fallbacks
- **Cost Overruns**: Hard limits on tool iterations and timeouts
- **Security**: Sandboxing, file type filtering, size limits

## Dependencies & Integration

### Core Dependencies
- **genai**: Multi-provider LLM client (Ollama, Claude, OpenAI, etc.)
- **tokio**: Async runtime
- **serde/serde_json**: Serialization
- **clap**: CLI framework
- **anyhow/thiserror**: Error handling

### Integration Points
- **CI/CD Systems**: Auto-configure build pipelines
- **IDEs**: Development environment setup
- **Container Registries**: Optimize Dockerfile generation
- **Kubernetes**: Generate deployment manifests
- **BuildKit Frontends**: Generate LLB graphs for container builds
- **Platform Tools**: Build definition generation for deployments

## License & Distribution

- **License**: Apache 2.0 (permissive, commercial-friendly)
- **Distribution**:
  - Cargo crate: `cargo install aipack`
  - Binary releases: GitHub Releases
  - Docker image: `ghcr.io/diverofdark/aipack`
- **Documentation**:
  - README.md: Quick start
  - CLAUDE.md: Development guide
  - PRD-2.0.md: This document
  - API docs: cargo doc

## Conclusion

aipack 2.0 represents a fundamental shift from static pattern matching to **reasoning-driven repository intelligence**. By giving LLMs agency through tools, we create a system that can understand any repository structure through exploration and reasoning rather than rules.

This approach unlocks:
- **Universal applicability**: Works with any build system
- **Monorepo intelligence**: Automatically discovers all projects
- **Adaptive behavior**: Adjusts exploration based on findings
- **Future-proof design**: Handles novel frameworks without updates

The tool-based architecture positions aipack as a foundation for intelligent developer tooling that can reason about codebases rather than just parse them.
