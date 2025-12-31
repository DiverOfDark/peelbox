# Change: Add LLM-Based Dynamic Type Detection

## Why

The current stack detection system uses hardcoded enums (LanguageId, BuildSystemId, FrameworkId, OrchestratorId) that require code changes to support new technologies. This creates several limitations:

1. **Limited to hardcoded types**: Can only detect the 13 languages, 16 build systems, 20 frameworks, and 3 orchestrators explicitly defined in enums
2. **Requires code updates for new tech**: Adding support for Zig, Bun 2.0, or SvelteKit requires modifying Rust source code and recompiling
3. **Poor fit for emerging ecosystems**: Cannot detect experimental frameworks, custom build tools, or organization-specific technologies
4. **Missed detection opportunities**: Projects using non-mainstream tech show up as "unknown" instead of providing LLM-discovered metadata

**Example**: A repository using Deno + Fresh framework currently fails detection because neither is in our hardcoded enums, even though the LLM could identify and describe them.

## What Changes

### 1. Add "Custom" Enum Variants

Extend all ID enums to support LLM-discovered types:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LanguageId {
    // Known types (compile-time validated)
    Rust,
    Java,
    // ... existing variants

    // LLM-discovered type (runtime-provided)
    Custom(String),  // e.g., "Zig", "V", "Nim"
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BuildSystemId {
    // Known types
    Cargo,
    Maven,
    // ... existing variants

    // LLM-discovered type
    Custom(String),  // e.g., "Bazel", "Buck2", "Pants"
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FrameworkId {
    // Known types
    SpringBoot,
    Express,
    // ... existing variants

    // LLM-discovered type
    Custom(String),  // e.g., "Fresh", "Qwik", "Solid"
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OrchestratorId {
    // Known types
    Turborepo,
    Nx,
    Lerna,

    // LLM-discovered type
    Custom(String),  // e.g., "Moon", "Bit"
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuntimeId {
    // Known types
    JVM,
    Node,
    Python,
    // ... existing variants

    // LLM-discovered type (NEW)
    Custom(String),  // e.g., "Deno", "Bun", "GraalVM"
}
```

### 2. Implement LLM-Backed Trait Implementations

Create Llm* structs that implement traits and call LLM on-demand:

```rust
/// LLM-backed language detection (calls LLM on first .detect())
pub struct LlmLanguage {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<LanguageInfo>>>,
}

impl LanguageDefinition for LlmLanguage {
    fn detect(&self, manifest_path: &Path, content: &str) -> bool {
        let mut info = self.detected_info.lock().unwrap();
        if info.is_none() {
            // Call LLM (blocks on async)
            match tokio::runtime::Handle::current()
                .block_on(self.llm_client.identify_language(manifest_path, content)) {
                Ok(detected) => *info = Some(detected),
                Err(_) => return false,
            }
        }
        true
    }

    fn id(&self) -> LanguageId {
        LanguageId::Custom(self.detected_info.lock().unwrap().as_ref().unwrap().name.clone())
    }

    // Other methods return LLM-discovered data
}

/// LLM-backed build system (similar pattern)
pub struct LlmBuildSystem {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<BuildSystemInfo>>>,
}

/// LLM-backed framework
pub struct LlmFramework {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<FrameworkInfo>>>,
}

/// LLM-backed runtime (NEW)
pub struct LlmRuntime {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<RuntimeInfo>>>,
}

/// LLM-backed orchestrator
pub struct LlmOrchestrator {
    llm_client: Arc<dyn LLMClient>,
    detected_info: Arc<Mutex<Option<OrchestratorInfo>>>,
}
```

### 3. Register Llm* Implementations in StackRegistry

Update StackRegistry to register Llm* implementations LAST (as fallback).

```rust
impl StackRegistry {
    pub fn with_defaults(llm_client: Option<Arc<dyn LLMClient>>) -> Self {
        let mut registry = Self::new();

        // Register known languages (deterministic, tried first)
        registry.register_language(Arc::new(RustLanguage));
        registry.register_language(Arc::new(JavaLanguage));
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

**Rationale**: No phase changes needed - phases already iterate registry implementations in order

## Impact

### Affected Specs

- **stack-registry** (new): Type system with Custom variants and LLM fallback
- **refactor-analysis-architecture** (completed): Pipeline now uses 5 phases, this change integrates with Phase 2 (WorkspaceStructure) and Phase 4 (ServiceAnalysis → StackIdentificationPhase)

### Affected Code

**New files**:
- `src/stack/llm/mod.rs` - Module for LLM-backed implementations (~50 lines)
- `src/stack/llm/language.rs` - LlmLanguage implementation (~150 lines)
- `src/stack/llm/buildsystem.rs` - LlmBuildSystem implementation (~150 lines)
- `src/stack/llm/framework.rs` - LlmFramework implementation (~150 lines)
- `src/stack/llm/runtime.rs` - LlmRuntime implementation (~150 lines, NEW)
- `src/stack/llm/orchestrator.rs` - LlmOrchestrator implementation (~100 lines)

**Modified files**:
- `src/stack/mod.rs` - Add `Custom(String)` to all ID enums including RuntimeId (~60 lines changed)
- `src/stack/registry.rs` - Update `with_defaults()` to optionally register Llm* implementations (~30 lines changed)
- `src/llm/client.rs` - Add identification methods to trait (~100 lines added):
  - `identify_language()`
  - `identify_build_system()`
  - `identify_framework()`
  - `identify_runtime()`
  - `identify_orchestrator()`
- `src/llm/genai.rs` - Implement new LLMClient methods (~200 lines added)
- `src/llm/mock.rs` - Implement new methods for MockLLMClient (~50 lines added)

**LLM prompts** (new):
- `identify_language` - JSON schema for language detection
- `identify_build_system` - JSON schema for build system detection
- `identify_framework` - JSON schema for framework detection
- `identify_runtime` - JSON schema for runtime detection
- `identify_orchestrator` - JSON schema for orchestrator detection

**PHASE CLEANUP** - Remove LLM calls from phases:
- Remove `llm_client` from AnalysisContext and ServiceContext
- Remove LLM fallback logic from RuntimeConfigPhase (currently has llm.chat() call)
- Remove `llm_helper.rs`
- Phases become pure orchestration (iterate registry, call trait methods)

### Breaking Changes

**Enum changes** (backward compatible via serde):
```rust
// Before: Only known variants
LanguageId::Rust | LanguageId::Java | ...

// After: Known + Custom
LanguageId::Rust | LanguageId::Java | ... | LanguageId::Custom(String)
```

**Pattern matching**:
```rust
// Before: Exhaustive match was possible
match language_id {
    LanguageId::Rust => "Cargo",
    LanguageId::Java => "Maven or Gradle",
    // ... all 13 variants
}

// After: Must handle Custom variant
match language_id {
    LanguageId::Rust => "Cargo",
    LanguageId::Custom(name) => &format!("Unknown ({})", name),
    _ => "Other",
}
```

**JSON serialization** (backward compatible):
```json
// Known type: serializes as before
{"language": "rust", "build_system": "cargo"}

// Custom type: serializes as string
{"language": "zig", "build_system": "zig-build"}
```

### Migration Path

1. Add `Custom(String)` variants to all enums in `src/stack/mod.rs` (including RuntimeId)
2. Add LLM response schemas (LanguageInfo, BuildSystemInfo, FrameworkInfo, RuntimeInfo, OrchestratorInfo)
3. Add identification methods to LLMClient trait
4. Implement Llm* trait implementations (`src/stack/llm/*`)
5. Update `StackRegistry::with_defaults()` to optionally register Llm* implementations
6. **CLEAN UP PHASES** - remove all LLM calls:
   - Remove `llm_client` from AnalysisContext and ServiceContext
   - Remove LLM fallback from RuntimeConfigPhase
   - Delete `llm_helper.rs`
7. Update all pattern matches to handle `Custom` variant
8. Add tests for Llm* implementations
9. Add fixtures for unknown technologies (Bazel, Zig, etc.)
10. Update recording system to capture LLM identification calls

**Key Benefit**:
- ~500 lines of code vs ~1000+ lines with phase-level fallback logic
- Phases become pure orchestration (no LLM awareness)

### Performance Impact

**Positive**:
- No LLM calls for known tech (fast path unchanged)
- Custom types cached in registry after first detection

**Negative**:
- Unknown tech triggers LLM calls (~200-500ms per type)
- Larger enum size (String variant adds heap allocation)

**Mitigation**:
- Only fallback to LLM when pattern matching fails (rare case)
- Cache custom types in registry to avoid repeated LLM calls
- Add configuration flag to disable LLM fallback if needed

### Risk Assessment

**Low risk**:
- Backward compatible JSON serialization (serde untagged)
- Known types use existing code paths (zero behavior change)
- Custom types isolated in separate module

**Medium risk**:
- All pattern matches must handle `Custom` variant (compilation enforced)
- Hash/Eq implementations may behave differently with String variants
- Custom type quality depends on LLM accuracy

**Mitigation**:
- Clippy lint for non-exhaustive pattern matches
- Integration tests for custom type detection
- Confidence scores to flag low-quality LLM responses

## Testing Strategy

The implementation includes three detection modes for comprehensive testing:

| Mode | Environment Variable | Purpose |
|------|---------------------|---------|
| **Full** | `PEELBOX_DETECTION_MODE=full` | Default - deterministic first, LLM fallback |
| **Static** | `PEELBOX_DETECTION_MODE=static` | Fast CI - deterministic only, no LLM |
| **LLM-only** | `PEELBOX_DETECTION_MODE=llm_only` | Validate LLM* implementations |

**LLM-only mode** (NEW):
- Registry registers ONLY LLM* implementations (no deterministic Rust, Java, npm, etc.)
- Forces all detection through LLM code path
- Validates that LLM* implementations correctly detect mainstream tech (not just unknowns)
- Each fixture gets three test variants: `test_*_full()`, `test_*_static()`, `test_*_llm_only()`

Benefits:
- ✅ Both deterministic and LLM code paths validated independently
- ✅ LLM quality assurance for known tech (ensures prompts work correctly)
- ✅ Recording mode captures LLM responses for mainstream languages/build systems
- ✅ CI can run static mode (fast, no LLM backend) and LLM-only mode (comprehensive)

## Success Criteria

✅ All existing tests pass with `Custom` variants added
✅ Pattern-based detection still works without LLM calls
✅ LLM* implementations correctly identify both known and unknown technologies
✅ Custom* types implement traits correctly
✅ Runtime registration works for LLM-discovered types
✅ JSON serialization backward compatible
✅ All pattern matches handle `Custom` variant
✅ Recording system captures LLM calls from LLM* implementations
✅ Performance neutral for known tech (no regression)
✅ Documentation explains when LLM fallback triggers
✅ LLM-only mode validates LLM* implementations work for mainstream tech
