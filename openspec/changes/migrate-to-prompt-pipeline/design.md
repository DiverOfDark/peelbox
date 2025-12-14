# Design: Prompt Pipeline Architecture

## Context

The current tool-based agentic architecture has the LLM explore repositories iteratively using 7 tools:

```rust
// Current: LLM-driven exploration loop in AnalysisPipeline
loop {
    message = llm.chat(messages);
    if message.is_tool_call() {
        result = tool_system.execute(message.tool_call);
        messages.push(result);
    } else if message.is_submit() {
        return parse_universal_build(message);
    }
}
```

**Current infrastructure that works well:**
- `DetectionService` - Clean public API
- `LLMClient` trait - Pluggable LLM backends (Ollama, Claude, embedded)
- `FileSystem` trait - Testable file operations
- `BootstrapScanner` - Pre-scan for language detection
- `LanguageRegistry` - Language definitions for 10+ languages
- `RecordingLLMClient` - Deterministic testing via recorded responses
- `ToolSystem` - 7 tools (list_files, read_file, search_files, get_file_tree, grep_content, get_best_practices, submit_detection)
- Test fixtures - Comprehensive fixtures for Rust, Node, Python, Java, Go, .NET, monorepos

**Fundamental limits of current approach:**
- Context accumulates (7 tool schemas + history + file contents)
- LLM decides exploration strategy, unpredictable token usage
- Sequential execution, no parallelization (even for monorepos)
- Cannot optimize small models (<3B parameters) due to reasoning requirements
- No path to extract heuristics and skip LLM for common patterns

## Goals

1. **Predictable token usage** - Fixed number of prompts, each <500 tokens
2. **Support smallest models** - All prompts fit in 8k context window
3. **Parallelization** - Independent phases run concurrently
4. **Deterministic when possible** - Known formats bypass LLM
5. **Optimizable** - Logging infrastructure enables future heuristic extraction
6. **Equivalent accuracy** - Matches or exceeds current tool-based approach

## Non-Goals

- Real-time streaming analysis (batch processing is fine)
- Supporting arbitrary custom prompts (fixed pipeline for v1)
- Backwards compatibility with tool-based internals

## Architecture

### Pipeline Phases

The pipeline consists of 10 phases, with 9 distinct LLM prompts:

```
Phase 1: Scan (no LLM)
         ↓
Phase 2: Classify Directories (Prompt 0) - 500 tokens
         ↓
Phase 3: Project Structure (Prompt 1) - 300 tokens
         ↓
Phase 4: Dependency Extraction (Prompt 2 fallback) - 200 tokens
         ↓
Phase 5: Build Order (no LLM)
         ↓
Phase 6: Service Analysis (sequential, per service)
         For each service:
           6a: Runtime Detection (Prompt 3) - 200 tokens
           6b: Build Detection (Prompt 4) - 150 tokens
           6c: Entrypoint Detection (Prompt 5) - 150 tokens
           6d: Native Dependencies (Prompt 6) - 150 tokens
           6e: Port Discovery (Prompt 7) - 150 tokens
           6f: Environment Variables (Prompt 8) - 200 tokens
           6g: Health Check Discovery (Prompt 9) - 150 tokens
         ↓
Phase 7: Cache Detection (no LLM - deterministic based on build system)
         ↓
Phase 8: Root Cache (no LLM - deterministic based on monorepo tool)
         ↓
Phase 9: Assemble (no LLM) → Returns UniversalBuild JSON
```

**Total tokens per detection:**
- Minimum: ~1,000 tokens (single service, all deterministic)
- Typical: ~2,000 tokens (single service, some LLM fallbacks)
- Maximum: ~6,000 tokens (monorepo with 3 services, all LLM phases)

**Current approach:** 10,000-50,000 tokens per detection

**Note:** Initial implementation is fully sequential for simplicity. Parallelization can be added later as an optimization.

### Module Structure

**Existing modules to preserve:**
- `src/llm/` - `LLMClient` trait, GenAI backend, embedded client, recording system
- `src/fs/` - `FileSystem` trait, real + mock implementations
- `src/bootstrap/` - `BootstrapScanner` for initial language detection
- `src/languages/` - `LanguageRegistry` with 10+ language definitions
- `src/detection/` - `DetectionService` public API (unchanged)
- `src/validation/` - Validation system
- `src/config.rs` - Configuration management
- `tests/fixtures/` - Comprehensive test fixtures

**New modules:**

```
src/
├── pipeline/
│   ├── mod.rs              # PipelineOrchestrator (replaces AnalysisPipeline)
│   ├── config.rs           # (existing, may need updates)
│   ├── context.rs          # (existing, may need updates)
│   └── phases/
│       ├── mod.rs
│       ├── scan.rs         # Phase 1: Filesystem scan (no LLM)
│       ├── classify.rs     # Phase 2: Directory classification (prompt + execution)
│       ├── structure.rs    # Phase 3: Project structure (prompt + execution)
│       ├── dependencies.rs # Phase 4: Dependency extraction (prompt + execution)
│       ├── build_order.rs  # Phase 5: Topological sort (no LLM)
│       ├── runtime.rs      # Phase 6a: Runtime detection (prompt + execution)
│       ├── build.rs        # Phase 6b: Build detection (prompt + execution)
│       ├── entrypoint.rs   # Phase 6c: Entrypoint detection (prompt + execution)
│       ├── native_deps.rs  # Phase 6d: Native dependencies (prompt + execution)
│       ├── port.rs         # Phase 6e: Port discovery (prompt + execution)
│       ├── env_vars.rs     # Phase 6f: Environment variables (prompt + execution)
│       ├── health.rs       # Phase 6g: Health check discovery (prompt + execution)
│       ├── cache.rs        # Phase 7: Cache detection (prompt + execution)
│       ├── root_cache.rs   # Phase 8: Root cache (prompt + execution)
│       └── assemble.rs     # Phase 9: Config assembly → UniversalBuild (no LLM)
│
├── languages/              # Extend existing module
│   ├── mod.rs              # Add `parse_dependencies()` to `LanguageDefinition` trait
│   ├── rust.rs             # Add Cargo.toml parsing for dependencies
│   ├── javascript.rs       # Add package.json/pnpm-workspace.yaml parsing
│   ├── python.rs           # Add pyproject.toml/requirements.txt parsing
│   ├── go.rs               # Add go.mod parsing
│   ├── java.rs             # Add pom.xml/build.gradle parsing
│   └── registry.rs         # Already exists, use for dependency parsing too
│
├── extractors/
│   ├── mod.rs
│   ├── port.rs             # Port extraction from code/config
│   ├── env_vars.rs         # Environment variable extraction
│   └── health.rs           # Health check endpoint extraction
│
└── heuristics/
    ├── mod.rs
    ├── logger.rs           # Heuristic logging infrastructure
    └── matcher.rs          # Future: pattern matching for shortcuts
```

**Note:** Template/Dockerfile rendering is handled by `get_best_practices` tool, which returns build templates from `LanguageDefinition.build_template()`. The pipeline outputs `UniversalBuild` JSON, not Dockerfiles.

### Key Components

#### 1. PipelineOrchestrator

```rust
pub struct PipelineOrchestrator {
    llm_client: Arc<dyn LLMClient>,
    parsers: ParserRegistry,
    extractors: ExtractorRegistry,
    config: PipelineConfig,
}

impl PipelineOrchestrator {
    pub async fn execute(&self, repo_path: PathBuf) -> Result<UniversalBuild> {
        // Phase 1: Scan
        let scan_result = phases::scan::execute(&repo_path)?;

        // Phase 2: Classify
        let classify_result = phases::classify::execute(
            &self.llm_client,
            &scan_result,
        ).await?;

        // Phase 3: Structure
        let structure_result = phases::structure::execute(
            &self.llm_client,
            &scan_result,
            &classify_result,
        ).await?;

        // Phase 4: Dependencies
        let deps_result = phases::dependencies::execute(
            &self.llm_client,
            &self.parsers,
            &scan_result,
            &structure_result,
        ).await?;

        // Phase 5: Build order
        let build_order = phases::build_order::execute(&deps_result)?;

        // Phase 6: Sequential service analysis
        let mut services = Vec::new();
        for service in &structure_result.services {
            let analysis = self.analyze_service(service, &scan_result).await?;
            services.push(analysis);
        }

        // Phase 7: Cache detection (per service)
        let mut service_caches = Vec::new();
        for (idx, service) in structure_result.services.iter().enumerate() {
            let cache = phases::cache::execute(
                &self.llm_client,
                service,
                &services[idx],
            ).await?;
            service_caches.push(cache);
        }

        // Phase 8: Root cache (monorepos only)
        let root_cache = if structure_result.project_type == ProjectType::Monorepo {
            Some(phases::root_cache::execute(&self.llm_client, &scan_result).await?)
        } else {
            None
        };

        // Phase 9: Assemble into UniversalBuild
        phases::assemble::execute(
            structure_result,
            build_order,
            services,
            service_caches,
            root_cache,
        )
    }

    async fn analyze_service(&self, service: &Service, scan: &ScanResult) -> Result<ServiceAnalysis> {
        // Run 6a-6g sequentially
        let runtime = phases::runtime::execute(&self.llm_client, service, scan).await?;
        let build = phases::build::execute(&self.llm_client, service, scan).await?;
        let entrypoint = phases::entrypoint::execute(&self.llm_client, service, scan).await?;
        let native_deps = phases::native_deps::execute(&self.llm_client, service, scan).await?;
        let port = phases::port::execute(&self.llm_client, &self.extractors, service, scan).await?;
        let env_vars = phases::env_vars::execute(&self.llm_client, &self.extractors, service, scan).await?;
        let health = phases::health::execute(&self.llm_client, &self.extractors, service, scan).await?;

        Ok(ServiceAnalysis {
            runtime,
            build,
            entrypoint,
            native_deps,
            port,
            env_vars,
            health,
        })
    }
}
```

#### 2. Phase Implementation Pattern

Each phase file contains both prompt building and execution logic:

```rust
// Example: src/pipeline/phases/runtime.rs

// Input/output types
pub struct RuntimePhaseInput {
    pub service_path: PathBuf,
    pub file_list: Vec<PathBuf>,
    pub manifest_excerpt: Option<String>,
}

pub struct RuntimePhaseOutput {
    pub runtime: String,
    pub runtime_version: Option<String>,
    pub framework: Option<String>,
    pub confidence: Confidence,
}

// Prompt builder (private to this module)
fn build_prompt(input: &RuntimePhaseInput) -> String {
    format!(r#"
You detect the runtime and framework for a service.

Path: {}
Files: {}
Manifest: {}

Answer in JSON:
{{
  "runtime": "node" | "go" | "java" | "python" | "rust" | "static" | "unknown",
  "runtime_version": "<string or null>",
  "framework": "nextjs" | "remix" | "vite" | "express" | ... | "none" | "unknown",
  "confidence": "high" | "medium" | "low"
}}
"#,
        input.service_path.display(),
        input.file_list.iter().take(20).map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "),
        input.manifest_excerpt.as_deref().unwrap_or("None")
    )
}

// Public execution function
pub async fn execute(
    llm_client: &dyn LLMClient,
    service: &Service,
    scan: &ScanResult,
) -> Result<RuntimePhaseOutput> {
    // Build minimal input
    let input = RuntimePhaseInput {
        service_path: service.path.clone(),
        file_list: extract_relevant_files(&scan, &service.path),
        manifest_excerpt: extract_manifest_excerpt(&scan, &service.path),
    };

    // Build prompt (<200 tokens)
    let prompt = build_prompt(&input);

    // Call LLM
    let response = llm_client.chat(&prompt).await?;

    // Parse structured response
    let output: RuntimePhaseOutput = serde_json::from_str(&response)?;

    // Validate
    validate_runtime(&output)?;

    Ok(output)
}
```

#### 3. Deterministic Dependency Parser Example

Extend existing `LanguageDefinition` trait:

```rust
// src/languages/mod.rs
pub trait LanguageDefinition: Send + Sync {
    // ... existing methods ...

    /// Parse dependencies from manifest content
    fn parse_dependencies(
        &self,
        manifest_path: &Path,
        manifest_content: &str,
        all_internal_paths: &[PathBuf],
    ) -> Result<DependencyInfo> {
        // Default: no parsing implemented, return empty
        Ok(DependencyInfo {
            path: manifest_path.parent().unwrap().to_path_buf(),
            internal_deps: vec![],
            external_deps: vec![],
            detected_by: DetectionMethod::NotImplemented,
            confidence: Confidence::Low,
        })
    }
}
```

```rust
// src/languages/javascript.rs
impl LanguageDefinition for JavaScriptLanguage {
    // ... existing methods ...

    fn parse_dependencies(
        &self,
        manifest_path: &Path,
        manifest_content: &str,
        all_internal_paths: &[PathBuf],
    ) -> Result<DependencyInfo> {
        let parsed: PackageJson = serde_json::from_str(manifest_content)?;

        // Extract workspace references
        let internal_deps = if let Some(workspaces) = parsed.workspaces {
            self.resolve_workspaces(&workspaces, all_internal_paths)
        } else {
            vec![]
        };

        Ok(DependencyInfo {
            path: manifest_path.parent().unwrap().to_path_buf(),
            internal_deps,
            external_deps: parsed.dependencies.keys().cloned().collect(),
            detected_by: DetectionMethod::Deterministic,
            confidence: Confidence::High,
        })
    }
}
```

### Data Flow

#### Phase 1-3: Repository Structure

```rust
ScanResult {
    file_tree: Vec<PathBuf>,           // All files in repo
    potential_manifests: Vec<Manifest>, // Detected by filename pattern
}
    ↓
ClassifyResult {
    services: Vec<ServicePath>,         // Independently deployable
    packages: Vec<PackagePath>,         // Shared libraries
    root_is_service: bool,
    confidence: Confidence,
}
    ↓
StructureResult {
    project_type: ProjectType,          // Monorepo | SingleService
    monorepo_tool: Option<MonorepoTool>, // pnpm | yarn | turbo | nx | ...
    services: Vec<Service>,
    packages: Vec<Package>,
    confidence: Confidence,
}
```

#### Phase 4-5: Dependencies & Build Order

```rust
DependencyResult {
    dependencies: HashMap<PathBuf, DependencyInfo>,
}

DependencyInfo {
    path: PathBuf,
    internal_deps: Vec<PathBuf>,       // References to other services/packages
    build_system: String,               // npm, cargo, gradle, etc.
    detected_by: DetectionMethod,       // Deterministic | LLM
    confidence: Confidence,
}
    ↓
BuildOrderResult {
    build_order: Vec<PathBuf>,          // Topologically sorted
    has_cycle: bool,
}
```

#### Phase 6: Service Analysis

```rust
ServiceAnalysis {
    runtime: RuntimeInfo {
        runtime: String,
        runtime_version: Option<String>,
        framework: Option<String>,
        confidence: Confidence,
    },
    build: BuildInfo {
        build_cmd: Option<String>,
        output_dir: Option<PathBuf>,
        confidence: Confidence,
    },
    entrypoint: EntrypointInfo {
        entrypoint: String,
        confidence: Confidence,
    },
    native_deps: NativeDepsInfo {
        needs_build_deps: bool,
        has_native_modules: bool,
        has_prisma: bool,
        native_deps: Vec<String>,
        confidence: Confidence,
    },
    port: PortInfo {
        port: Option<u16>,
        from_env: bool,
        env_var: Option<String>,
        confidence: Confidence,
    },
    env_vars: EnvVarsInfo {
        env_vars: Vec<EnvVar>,
        confidence: Confidence,
    },
    health: HealthInfo {
        health_endpoints: Vec<HealthEndpoint>,
        recommended_liveness: Option<String>,
        recommended_readiness: Option<String>,
        confidence: Confidence,
    },
}
```

#### Phase 9: Final Assembly

```rust
AssembledConfig {
    project_type: ProjectType,
    monorepo_tool: Option<MonorepoTool>,
    build_order: Vec<PathBuf>,
    services: Vec<ServiceConfig>,
    root_cache: Option<CacheConfig>,
}

ServiceConfig {
    path: PathBuf,
    runtime: String,
    runtime_version: Option<String>,
    framework: Option<String>,
    build_cmd: Option<String>,
    output_dir: Option<PathBuf>,
    entrypoint: String,
    port: Option<u16>,
    port_env_var: Option<String>,
    needs_build_deps: bool,
    has_prisma: bool,
    has_native_modules: bool,
    internal_deps: Vec<PathBuf>,
    env_vars: Vec<EnvVar>,
    cache_dependencies: Vec<PathBuf>,
    cache_build: Vec<PathBuf>,
    health: Option<HealthConfig>,
    confidence: ConfidenceScores,
}
```

### Sequential Execution Strategy

```rust
// Phase 6: Analyze each service sequentially
for service in &services {
    let analysis = analyze_service(service, &scan_result).await?;
    service_analyses.push(analysis);
}

// Within each service, run 6a-6g sequentially
async fn analyze_service(...) -> Result<ServiceAnalysis> {
    let runtime = phases::runtime::execute(...).await?;
    let build = phases::build::execute(...).await?;
    let entrypoint = phases::entrypoint::execute(...).await?;
    let native_deps = phases::native_deps::execute(...).await?;
    let port = phases::port::execute(...).await?;
    let env_vars = phases::env_vars::execute(...).await?;
    let health = phases::health::execute(...).await?;

    Ok(ServiceAnalysis { runtime, build, entrypoint, ... })
}
```

**Sequential execution benefits:**
- **Simpler code:** Easier to understand, debug, and maintain
- **Predictable behavior:** No race conditions or ordering issues
- **Lower resource usage:** One LLM call at a time
- **Future optimization:** Can add parallelization later once pipeline is proven stable

**Note:** While this is slower than parallel execution, it's still much faster than the current tool-based approach due to reduced token usage and fewer iterations.

### Deterministic Parsers

Parsers bypass LLM for known manifest formats:

```rust
pub enum DetectionMethod {
    Deterministic,  // Parsed without LLM
    LLM,           // LLM fallback used
}

// Example: Phase 4 dependency extraction
if manifest_type == "package.json" {
    deps = NodeParser::parse_dependencies(manifest, internal_paths)?;
    deps.detected_by = DetectionMethod::Deterministic;
    deps.confidence = Confidence::High;
} else {
    deps = llm_client.extract_dependencies(manifest, internal_paths).await?;
    deps.detected_by = DetectionMethod::LLM;
}
```

**Supported formats (Phase 4):**
- `package.json` (Node.js/npm/pnpm/yarn)
- `pnpm-workspace.yaml` (pnpm monorepos)
- `Cargo.toml` (Rust)
- `go.mod` (Go)
- `pom.xml` (Maven)
- `build.gradle`, `build.gradle.kts` (Gradle)
- `pyproject.toml`, `requirements.txt` (Python)

### Heuristic Logging

Each phase logs input/output for future optimization:

```rust
// src/heuristics/logger.rs
pub struct HeuristicLogger {
    repo_id: Uuid,
    log_file: PathBuf,
}

impl HeuristicLogger {
    pub fn log_phase<I, O>(&self, phase: &str, input: &I, output: &O, latency_ms: u64)
    where
        I: Serialize,
        O: Serialize,
    {
        let entry = LogEntry {
            repo_id: self.repo_id,
            timestamp: Utc::now(),
            phase: phase.to_string(),
            input_hash: hash(input),
            output_hash: hash(output),
            input: serde_json::to_value(input).unwrap(),
            output: serde_json::to_value(output).unwrap(),
            latency_ms,
        };

        // Write to JSONL file
        writeln!(self.log_file, "{}", serde_json::to_string(&entry).unwrap()).unwrap();
    }
}
```

**Future optimization path:**

1. Analyze logs to find patterns: "When `package.json` contains `"type": "module"` and files include `next.config.js`, runtime is always `node` and framework is always `nextjs`"

2. Extract to heuristics:
```rust
pub fn try_heuristic_runtime(input: &RuntimePhaseInput) -> Option<RuntimePhaseOutput> {
    if input.has_file("next.config.js") && input.manifest_has_field("type", "module") {
        return Some(RuntimePhaseOutput {
            runtime: "node".to_string(),
            framework: Some("nextjs".to_string()),
            detected_by: DetectionMethod::Heuristic,
            confidence: Confidence::High,
        });
    }
    None
}
```

3. Skip LLM when heuristic matches:
```rust
if let Some(result) = heuristics::try_runtime(input) {
    return Ok(result);
}
// Fall back to LLM
llm_client.detect_runtime(input).await
```

## Migration Strategy: Incremental Implementation

### Week 1-2: Foundation & Language Extensions

1. **Extend `LanguageDefinition` trait** with `parse_dependencies()`
2. **Implement dependency parsing** in each language (Rust, JS, Java, Python, Go)
3. **Create prompt builders module** (`src/prompts/`)
4. **Create extractors module** (`src/extractors/`)
5. **Unit test** each component independently

**Deliverable:** Language-specific dependency parsing working, extractors ready

### Week 3: Core Pipeline Phases (1-5)

1. **Refactor `AnalysisPipeline`** to use phase-based approach
2. **Implement Phase 1** (Scan) - leverage existing `BootstrapScanner`
3. **Implement Phase 2** (Classify) - with option to skip if Bootstrap has high confidence
4. **Implement Phase 3** (Structure) - determine monorepo vs single service
5. **Implement Phase 4** (Dependencies) - use language parsers, LLM fallback
6. **Implement Phase 5** (Build order) - topological sort
7. **Integration test** phases 1-5 with fixture repos

**Deliverable:** Phases 1-5 working, can detect project structure and dependencies

### Week 4: Service Analysis Phases (6a-6g, 7)

1. **Implement Phase 6a** (Runtime detection)
2. **Implement Phase 6b** (Build detection)
3. **Implement Phase 6c** (Entrypoint detection)
4. **Implement Phase 6d** (Native dependencies)
5. **Implement Phase 6e** (Port discovery)
6. **Implement Phase 6f** (Environment variables)
7. **Implement Phase 6g** (Health checks)
8. **Implement Phase 7** (Cache detection)
9. **Integration test** full service analysis

**Deliverable:** Complete service analysis working for single services

### Week 5: Monorepo Support & Assembly (8-9)

1. **Implement Phase 8** (Root cache for monorepos)
2. **Implement Phase 9** (Assemble into UniversalBuild)
3. **Test with monorepo fixtures** (npm-workspaces, cargo-workspace, etc.)
4. **Add heuristic logging** to all LLM phases
5. **Add progress reporting**

**Deliverable:** Full pipeline working for both single services and monorepos

### Week 6: Cleanup & Polish

1. **Remove tool infrastructure** (`src/tools/` - except `get_best_practices` if still needed)
2. **Remove tool-based conversation loop** from old `AnalysisPipeline`
3. **Update all tests** to use new pipeline
4. **Update documentation** (CLAUDE.md, README.md)
5. **Run full test suite** on all fixtures
6. **Update CHANGELOG**

**Deliverable:** Clean codebase, all tests passing, documentation updated

### Incremental Testing Strategy

- **After each phase:** Unit test the phase in isolation
- **After each week:** Integration test completed phases together
- **Use `RecordingLLMClient`:** Record responses for deterministic tests
- **Leverage existing fixtures:** Test against `tests/fixtures/` repos
- **No dual-mode:** Only implement new pipeline, remove old code as we go

## Success Criteria

The pipeline must achieve:

1. **Token reduction:** ≥80% reduction in average tokens per detection (measure against logs)
2. **Model support:** Works with Qwen 2.5 Coder 0.5B/1.5B (8k context)
3. **Deterministic coverage:** ≥60% of fixture repos use deterministic parsers for dependencies
4. **Test coverage:** All existing fixtures continue to pass (with updated recordings)
5. **Code simplicity:** Pipeline code is easier to understand than tool-based approach

## Trade-offs

**Advantages:**
- Predictable, low token usage
- Supports smallest models
- Parallelizable
- Optimizable over time
- Easier to debug

**Disadvantages:**
- Less flexible (fixed prompts vs adaptive exploration)
- May miss edge cases initially (can add targeted prompts)
- Larger upfront implementation effort
- More complex orchestration logic

**Mitigation:**
- Comprehensive test corpus to catch edge cases
- Incremental rollout with both approaches running in parallel
- Add targeted prompts for discovered edge cases
- Heuristic logging enables continuous improvement

## Future Enhancements

1. **Parallelization** - Run phases 6a-6g concurrently per service, run multiple services in parallel
2. **Heuristic extraction** - Automatically learn patterns from logs
3. **Prompt caching** - Cache prompt responses by input hash
4. **Confidence-based skipping** - Skip LLM phases when deterministic confidence is high
5. **Custom phases** - Allow users to add project-specific detection phases
6. **Multi-language support** - Translate prompts for non-English repos
7. **Streaming** - Stream results as phases complete (don't wait for all)
