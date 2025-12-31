# Design: LLM-Based Dynamic Type Detection

## Overview

This change extends the type-safe stack detection system to support LLM-discovered types while maintaining backward compatibility and compile-time safety for known technologies.

**Key Insight**: Instead of adding LLM fallback logic to pipeline phases, push LLM detection down into **trait implementations**. This keeps phase code clean and encapsulates LLM logic within the trait system.

**Current Architecture (Deterministic Only)**:
```rust
// StackRegistry contains known implementations
registry.languages = vec![
    Arc::new(RustLanguage),
    Arc::new(JavaLanguage),
    // ... 13 hardcoded languages
];

// StackIdentificationPhase tries each one
for lang in &registry.languages {
    if lang.detect(manifest_path, content) {
        return lang.id();  // Found!
    }
}
// No match = failure
```

**New Architecture (LLM-Backed Fallback)**:
```rust
// StackRegistry contains known + LLM fallback (registered last)
registry.languages = vec![
    Arc::new(RustLanguage),           // Deterministic (tries first)
    Arc::new(JavaLanguage),           // Deterministic
    // ... other known languages
    Arc::new(LlmLanguage::new(llm)),  // LLM-backed (fallback, tries last)
];

// Same phase code! No changes needed.
for lang in &registry.languages {
    if lang.detect(manifest_path, content) {
        return lang.id();  // Known OR LLM-discovered
    }
}
```

**This change**:
- **Clean separation**: LLM logic in trait implementations, not phases
- **Uniform interface**: Phases don't know if using deterministic or LLM detection
- **Lazy evaluation**: LLM only called when all deterministic methods fail (registered last)
- **Type safety**: Custom(String) variants with full trait implementations

## Key Design Decisions

### 1. LLM-Backed Trait Implementations

**Decision**: Create `LlmLanguage`, `LlmBuildSystem`, `LlmFramework`, `LlmRuntime`, `LlmOrchestrator` structs that implement the respective traits and call LLM on-demand.

**Alternatives Considered**:
- LLM fallback in phases → Rejected: Phases become complex, LLM logic scattered
- Separate `UnknownLanguage` types → Rejected: Breaks trait polymorphism
- String-based system with no traits → Rejected: Loses type safety

**Rationale**:
- ✅ **Phases stay clean**: No LLM logic in phase code, just calls `.detect()`
- ✅ **Uniform interface**: `LanguageDefinition` trait works for both deterministic and LLM
- ✅ **Lazy evaluation**: LLM only called when deterministic methods fail
- ✅ **Encapsulation**: LLM client and state management hidden in implementation

**Example - LlmLanguage**:
```rust
pub struct LlmLanguage {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<LanguageInfo>>>,
}

impl LlmLanguage {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self {
            llm_client,
            detected_info: Arc::new(Mutex::new(None)),
        }
    }
}

impl LanguageDefinition for LlmLanguage {
    fn detect(&self, manifest_path: &Path, content: &str) -> bool {
        // Call LLM if not already detected
        let mut info = self.detected_info.lock().unwrap();
        if info.is_none() {
            match self.llm_client.identify_language(manifest_path, content) {
                Ok(detected) => *info = Some(detected),
                Err(_) => return false,
            }
        }
        true // Always returns true after successful LLM call
    }

    fn id(&self) -> LanguageId {
        let info = self.detected_info.lock().unwrap();
        LanguageId::Custom(info.as_ref().unwrap().name.clone())
    }

    fn name(&self) -> &str {
        // Returns static "LLM" or discovered name via internal cache
        "LLM"
    }

    fn file_extensions(&self) -> &[&str] {
        // Returns LLM-discovered extensions
        // Implementation detail: uses thread-local storage or lazy_static
        &[]
    }
}
```

**Trade-offs**:
- ✅ No changes to phase code
- ✅ LLM logic encapsulated in one place
- ✅ Easy to test (mock LlmLanguage without phases)
- ❌ Slight complexity in trait implementation (needs internal state)
- ❌ Async detection in sync trait (needs runtime or blocking)

### 2. Detection Flow: Registry Ordering

**Decision**: StackRegistry registers LLM-backed implementations **last** in the detection order. Phases iterate through implementations in order, so deterministic ones are tried first.

**Alternatives Considered**:
- Explicit fallback in phases → Rejected: Phases become aware of LLM logic
- Random/parallel detection → Rejected: Non-deterministic, wastes LLM calls
- LLM-first approach → Rejected: Too slow for known technologies

**Rationale**:
- ✅ **Implicit fallback**: No special "if deterministic fails, try LLM" logic
- ✅ **Phases stay clean**: Just iterate registry, don't know about LLM
- ✅ **Predictable**: Deterministic always tried first (fast path)
- ✅ **Easy to test**: Remove LlmLanguage from registry = deterministic-only mode

**Example - StackRegistry Setup**:
```rust
impl StackRegistry {
    pub fn with_defaults(llm_client: Option<Arc<dyn LLMClient>>) -> Self {
        let mut registry = Self::new();

        // Register known languages (deterministic, tried first)
        registry.register_language(Arc::new(RustLanguage));
        registry.register_language(Arc::new(JavaLanguage));
        registry.register_language(Arc::new(PythonLanguage));
        // ... other known languages

        // Register LLM fallback LAST (only called if all above fail)
        if let Some(llm) = llm_client {
            registry.register_language(Arc::new(LlmLanguage::new(llm.clone())));
            registry.register_build_system(Arc::new(LlmBuildSystem::new(llm.clone())));
            registry.register_framework(Arc::new(LlmFramework::new(llm.clone())));
            registry.register_runtime(Arc::new(LlmRuntime::new(llm.clone())));
            registry.register_orchestrator(Arc::new(LlmOrchestrator::new(llm)));
        }

        registry
    }
}
```

**Example - Phase Code (Unchanged)**:
```rust
// StackIdentificationPhase - NO CHANGES NEEDED
pub fn detect_language(&self, context: &ServiceContext) -> Result<LanguageId> {
    let manifest_path = &context.service.manifest_path;
    let content = fs::read_to_string(manifest_path)?;

    // Try each registered language in order
    for lang in context.stack_registry().languages() {
        if lang.detect(manifest_path, &content) {
            return Ok(lang.id());  // Could be known OR LLM-discovered
        }
    }

    Err(anyhow!("No language detected"))
}
```

### 3. State Management in LLM Implementations

**Decision**: Use internal mutable state (`Arc<Mutex<Option<T>>>`) to cache LLM responses within the trait implementation.

**Alternatives Considered**:
- Global cache outside trait → Rejected: Breaks encapsulation, harder to test
- Make trait async → Rejected: Breaking change to existing codebase
- No caching, call LLM every time → Rejected: Too expensive

**Rationale**:
- ✅ Encapsulates LLM logic completely within implementation
- ✅ Single LLM call per instance (cached after first detection)
- ✅ Works with existing sync trait interface

**Example - LlmBuildSystem**:
```rust
pub struct LlmBuildSystem {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<BuildSystemInfo>>>,
}

impl BuildSystem for LlmBuildSystem {
    fn detect(&self, manifest_path: &Path, content: &str) -> bool {
        let mut info = self.detected_info.lock().unwrap();
        if info.is_none() {
            // Block on async LLM call (use tokio::runtime::Handle::current().block_on)
            let result = tokio::runtime::Handle::current()
                .block_on(self.llm_client.identify_build_system(manifest_path, content));

            match result {
                Ok(detected) => *info = Some(detected),
                Err(_) => return false,
            }
        }
        true
    }

    fn cache_directories(&self) -> Vec<String> {
        self.detected_info.lock().unwrap()
            .as_ref()
            .map(|info| info.cache_dirs.clone())
            .unwrap_or_default()
    }

    fn id(&self) -> BuildSystemId {
        let info = self.detected_info.lock().unwrap();
        BuildSystemId::Custom(info.as_ref().unwrap().name.clone())
    }
}
```

### 4. Confidence Thresholds

**Decision**: Reject LLM identifications below 50% confidence.

**Rationale**:
- Low-confidence responses indicate LLM uncertainty (better to fail than guess)
- Prevents garbage custom types from being used
- Users can inspect failure and add hardcoded support if needed

**Thresholds**:
- `< 0.5` → Reject (too uncertain)
- `0.5 - 0.7` → Accept with warning log
- `> 0.7` → Accept silently

### 5. Serialization Format: Untagged Enums

**Decision**: Use `#[serde(untagged)]` for backward-compatible JSON output.

**Rationale**:
- Known types serialize as lowercase strings: `"rust"`, `"cargo"`
- Custom types serialize as-is: `"zig"`, `"bazel"`
- No JSON schema change (consumers see strings, not tagged variants)

**Example**:
```json
// Known type
{"language": "rust", "build_system": "cargo"}

// Custom type (indistinguishable in JSON)
{"language": "zig", "build_system": "bazel"}
```

### 6. LLM Logic in Trait Implementations

**Decision**: LLM* implementations (LLMRuntime, LLMLanguage, etc.) contain `llm_client` and call LLM internally within existing trait methods. **No trait changes needed**.

**LLM* Structs Hold LLM Client**:
```rust
// LLMRuntime holds llm_client
pub struct LLMRuntime {
    llm_client: Arc<dyn LLMClient>,
}

impl LLMRuntime {
    pub fn new(llm_client: Arc<dyn LLMClient>) -> Self {
        Self { llm_client }
    }
}

// Implements existing Runtime trait - calls LLM internally
impl Runtime for LLMRuntime {
    fn name(&self) -> &str {
        "LLM"
    }

    fn try_extract(&self, files: &[PathBuf], framework: Option<&dyn Framework>) -> Option<RuntimeConfig> {
        // Build prompt specific to runtime config extraction
        let prompt = self.build_runtime_prompt(files, framework)?;

        // Block on async LLM call
        let response = tokio::runtime::Handle::current()
            .block_on(self.llm_client.chat(prompt))
            .ok()?;

        // Parse LLM response into RuntimeConfig
        self.parse_runtime_response(&response).ok()
    }

    fn runtime_base_image(&self, version: Option<&str>) -> String {
        // Returns LLM-discovered base image or default
        "alpine:latest".to_string()
    }

    // Other methods...
}

// Similar for LLMLanguage, LLMBuildSystem, etc.
pub struct LLMLanguage {
    llm_client: Arc<dyn LLMClient>,
}

impl LanguageDefinition for LLMLanguage {
    fn detect(&self, manifest_path: &Path, content: &str) -> bool {
        // Internally calls LLM
        let prompt = self.build_language_prompt(manifest_path, content);
        tokio::runtime::Handle::current()
            .block_on(self.llm_client.chat(prompt))
            .is_ok()
    }
}
```

**Key Points**:
- **No trait changes**: Existing `try_extract()`, `detect()` methods work as-is
- **LLM client ownership**: Each LLM* struct owns `Arc<dyn LLMClient>`
- **Internal prompting**: LLM* implementations build prompts and parse responses
- **Blocking on async**: Use `tokio::runtime::Handle::current().block_on()` in sync traits

**Phases Stay Clean** (no changes needed):
```rust
// RuntimeConfigPhase - NO CHANGES
for runtime in runtimes {
    if let Some(config) = runtime.try_extract(files, framework) {
        return Ok(config);  // Could be JvmRuntime OR LLMRuntime
    }
}
// Phase has NO idea which implementation used LLM
```

## Data Flow

```
User Request
    │
    ▼
DetectionService
    │
    ├─ Create LLM client (Ollama/Claude/etc.)
    └─ Create StackRegistry with LLM fallbacks
           registry.register_language(LlmLanguage::new(llm))
           registry.register_build_system(LlmBuildSystem::new(llm))
           // ... other Llm* implementations
    │
    ▼
PipelineOrchestrator::execute()
    │
    ▼
Phase 1: Scan
    │
    └─ Returns: ScanResult with file list and manifest paths
    │
    ▼
Phase 2: WorkspaceStructure
    │
    └─ Iterate registry.orchestrators() (NO PHASE-LEVEL LLM LOGIC)
           ├─ TurborepoOrchestrator.detect() → false
           ├─ NxOrchestrator.detect() → false
           ├─ LernaOrchestrator.detect() → false
           └─ LlmOrchestrator.detect() → true (calls LLM internally)
                   │
                   └─ Internal: LLMClient.identify_orchestrator(...) → OrchestratorInfo
                   └─ Returns: OrchestratorId::Custom("moon")
    │
    └─ Returns: WorkspaceStructure { orchestrator, packages }
    │
    ▼
Phase 3: RootCache
    │
    └─ Detects root-level cache directories
    │
    ▼
Phase 4: ServiceAnalysis (per service/package)
    │
    ├─ StackIdentificationPhase (NO PHASE-LEVEL LLM LOGIC)
    │      │
    │      ├─ Iterate registry.languages()
    │      │      ├─ RustLanguage.detect() → false
    │      │      ├─ JavaLanguage.detect() → false
    │      │      └─ LlmLanguage.detect() → true (calls LLM internally)
    │      │             │
    │      │             └─ Internal: LLMClient.identify_language(...) → LanguageInfo
    │      │             └─ Returns: LanguageId::Custom("zig")
    │      │
    │      ├─ Iterate registry.build_systems()
    │      │      └─ LlmBuildSystem.detect() → true
    │      │             └─ Returns: BuildSystemId::Custom("zig-build")
    │      │
    │      ├─ Iterate registry.frameworks()
    │      │      └─ Returns: None or FrameworkId::Custom("...")
    │      │
    │      └─ Iterate registry.runtimes()
    │             └─ LlmRuntime.detect() → true
    │                    └─ Returns: RuntimeId::Custom("native")
    │      │
    │      └─ Returns: Stack { all typed IDs }
    │
    ├─ BuildPhase
    │      └─ Uses build_system methods (works with Llm* implementations)
    │
    ├─ RuntimeConfigPhase
    │      └─ Uses runtime methods (works with Llm* implementations)
    │
    └─ CachePhase
           └─ Calls build_system.cache_directories() (Llm* returns LLM-provided data)
    │
    ▼
Phase 5: Assemble
    │
    └─ Combine into UniversalBuild (Custom IDs serialize as strings in JSON)

**Key Difference**: LLM logic is INSIDE trait implementations (Llm*), NOT in phases.
Phases just iterate implementations and call .detect() - they don't know about LLM.
```

## Error Handling

### Low Confidence Response
```rust
if response.confidence < 0.5 {
    return Err(anyhow!(
        "LLM identification confidence too low ({:.2}). \
         Cannot reliably detect build system.",
        response.confidence
    ));
}
```

### Missing Required Fields
```rust
if response.name.trim().is_empty() {
    return Err(anyhow!("LLM response missing required 'name' field"));
}
```

## Testing Strategy

### Unit Tests
- ID enum serialization/deserialization (Known vs Custom struct variants)
- Pattern match exhaustiveness (compiler enforces handling Custom variant)
- Metadata extraction from Custom variants
- Confidence validation for LLM responses
- `name()` method returns correct value for Custom variant

### Integration Tests
- Pattern detection bypasses LLM (verify no LLM calls)
- LLM fallback triggers on unknown manifest
- Custom metadata flows through pipeline phases
- Custom variants serialize correctly in JSON output
- Multiple custom types in same project
- Downstream phases (Cache, Build) correctly consume Custom variant metadata

### Fixtures
- `tests/fixtures/edge-cases/bazel-build/` - Unknown build system
- `tests/fixtures/single-language/zig-build/` - Unknown language
- `tests/fixtures/single-language/deno-fresh/` - Unknown framework

### Recording System
- Add schemas for identification responses
- Record LLM calls for custom type detection
- Replay in tests without live API

## Performance Characteristics

| Scenario | Pattern Detection | LLM Fallback | Total |
|----------|------------------|--------------|-------|
| Known tech (Cargo, npm, Maven) | ~5ms | Not called | ~5ms |
| Unknown tech (Bazel, Zig) | ~5ms (fail) | ~200-500ms | ~205-505ms |

**Note**: Each detection is independent - custom types are not cached globally. If the same unknown tech appears in multiple services, LLM will be called for each occurrence. Future optimization could cache custom types within a single detection run.

## Testing Strategy

### Detection Modes

The system supports three detection modes for comprehensive testing:

| Mode | Environment Variable | Behavior | Use Case |
|------|---------------------|----------|----------|
| **Full** | `PEELBOX_DETECTION_MODE=full` | Deterministic first, LLM fallback | Normal operation (default) |
| **Static** | `PEELBOX_DETECTION_MODE=static` | Deterministic only, no LLM | Fast CI, validate parsers |
| **LLM-only** | `PEELBOX_DETECTION_MODE=llm_only` | LLM only, skip deterministic | Validate LLM* implementations |

**LLM-only mode** (NEW):
- StackRegistry registers ONLY LLM* implementations (skips Rust, Java, npm, etc.)
- Forces all detection through LLM code path
- Validates LLM* can detect BOTH known and unknown tech
- Tests that LLM prompts correctly identify mainstream languages/build systems

Example test structure:
```rust
#[test]
fn test_rust_cargo_full() {
    // Default: RustLanguage tries first (succeeds), LLMLanguage never called
}

#[test]
fn test_rust_cargo_static() {
    // PEELBOX_DETECTION_MODE=static: Only RustLanguage, no LLM backend needed
}

#[test]
fn test_rust_cargo_llm_only() {
    // PEELBOX_DETECTION_MODE=llm_only: ONLY LLMLanguage registered
    // Validates LLM can correctly identify Rust + Cargo
}
```

### Benefits

- **Full coverage**: Both deterministic and LLM code paths validated
- **LLM quality assurance**: Ensures LLM* implementations work for mainstream tech (not just unknowns)
- **Independent validation**: Each detection mode tested in isolation
- **Recording compatibility**: LLM-only mode captures responses for mainstream tech

## Migration Path

1. **Add Custom(String) variants** to all ID enums (LanguageId, BuildSystemId, FrameworkId, OrchestratorId, RuntimeId)
2. **Update name() methods** to handle Custom variant (return inner String)
3. **Extend LLM* implementations** (already exist, need LLM logic):
   - `src/stack/runtime/llm.rs` - Extend LLMRuntime with llm_client field and prompt logic
   - `src/stack/language/llm.rs` - Create LLMLanguage (new file)
   - `src/stack/buildsystem/llm.rs` - Create LLMBuildSystem (new file)
   - `src/stack/framework/llm.rs` - Create LLMFramework (new file)
   - `src/stack/orchestrator/llm.rs` - Create LLMOrchestrator (new file)
4. **Each LLM* implementation contains**:
   - `llm_client: Arc<dyn LLMClient>` field
   - `new(llm_client)` constructor
   - Internal prompt building methods (e.g., `build_runtime_prompt()`)
   - Internal response parsing methods (e.g., `parse_runtime_response()`)
   - Trait implementation that calls LLM in `try_extract()`/`detect()`
5. **Update StackRegistry::with_defaults()** to register LLM* implementations:
   ```rust
   if let Some(llm) = llm_client {
       registry.register_runtime(Arc::new(LLMRuntime::new(llm.clone())));
       registry.register_language(Arc::new(LLMLanguage::new(llm.clone())));
       // ... etc
   }
   ```
6. **Add detection mode support** to StackRegistry:
   - Check `PEELBOX_DETECTION_MODE` environment variable
   - `llm_only`: Register ONLY LLM* implementations
   - `static`: Skip LLM* registration entirely
   - `full`: Register deterministic first, then LLM* (default)
7. **NO PHASE CHANGES NEEDED** - phases already iterate registry and call trait methods
8. **Fix pattern matches** to handle Custom variants (compiler will warn)
9. **Add tests** for LLM* implementations (mock LLMClient)
10. **Add fixtures** for unknown technologies (Bazel, Zig, Fresh, etc.)
11. **Add LLM-only test variants** for all existing fixtures
12. **Update recording system** to capture LLM calls made from within trait implementations

**Key Benefits**:
- ✅ **Phases unchanged** - already work correctly
- ✅ **LLM logic encapsulated** in LLM* trait implementations
- ✅ **Easy to test** - mock LLMClient in LLM* structs
- ✅ **Clean separation** - deterministic vs LLM implementations

**What This Achieves**:
- LLMRuntime internally calls LLM in `try_extract()` → phases don't know
- LLMLanguage internally calls LLM in `detect()` → phases don't know
- Registry ordering ensures LLM* tried last (implicit fallback)
- No `if deterministic_failed { call_llm() }` logic in phases

No breaking changes to external JSON API (Custom variants serialize as strings).

## Future Enhancements

### 1. Learning System
Store successful custom type detections and promote to hardcoded types after confidence threshold (e.g., 10 successful detections).

### 2. User-Provided Definitions
Allow users to provide custom type definitions in config file (bypass LLM for known-internal tech).

### 3. Confidence Weighting
Use confidence scores to rank multiple detection results.

### 4. Type Suggestions
When LLM detects similar custom type, suggest mapping to known type (e.g., "npm-workspaces" → "npm").
