# Design: LLM-Based Dynamic Type Detection

## Overview

This change extends the type-safe stack detection system to support LLM-discovered types while maintaining backward compatibility and compile-time safety for known technologies.

**Key Insight**: The pipeline already uses LLM prompts in phases 2-12 that return type names as strings (e.g., `"framework": "nextjs"`, `"orchestrator": "turborepo"`). Rather than creating entirely new LLM identification methods, we enhance these existing prompts to:
1. Accept responses with metadata fields (backward compatible)
2. Map string names to typed enum variants
3. Create custom types when LLM returns unknown names

**Current LLM Usage**:
- Phase 2 (Classify): Classifies dirs as service/package
- Phase 3 (Structure): Returns `"orchestrator": "turborepo" | "nx" | ...` as strings ← **Enhance this**
- Phase 6 (Runtime): Returns `"framework": "nextjs" | "express" | ...` as strings ← **Enhance this**
- Phase 7 (Build): Returns build commands (implicitly tied to BuildSystemId)
- Phases 8-12: Return config values (entrypoint, native deps, port, env vars, health check)

**This change**:
- Adds type safety: String names validated against enums, compiler-enforced pattern matching
- Enables extensibility: Unknown names create Custom(String) variants with metadata
- Minimal prompt changes: Existing prompts enhanced, not replaced (preserves validation data)

## Key Design Decisions

### 1. Enum Extension Strategy: Custom Struct Variant

**Decision**: Add `Custom { name, metadata... }` struct variant to existing enums rather than creating separate "unknown type" structs.

**Alternatives Considered**:
- Separate `UnknownLanguage`, `UnknownBuildSystem` structs → Rejected: Requires two code paths for every operation
- String-based system with no enums → Rejected: Loses type safety and compile-time validation
- Generic `Type<T>` wrapper → Rejected: Overly complex for simple use case
- Separate `*Info` wrapper enums → Rejected: Unnecessary indirection, adds complexity
- `Custom(String)` variant → Rejected: Loses metadata needed by later phases

**Rationale**:
- Single enum handles both known and unknown types uniformly
- Metadata stored directly in Custom struct variant (no separate types needed)
- Rust compiler enforces handling `Custom` variant in pattern matches
- Natural fit: `Cargo` and `Custom { name: "bazel", ... }` are both build systems

**Example**:
```rust
pub enum BuildSystemId {
    Cargo,
    Maven,
    Gradle,
    // ... other known types

    Custom {
        name: String,
        manifest_files: Vec<String>,
        build_commands: Vec<String>,
        cache_dirs: Vec<String>,
    },
}
```

**Trade-offs**:
- ❌ Enums can no longer be `Copy` (struct variants contain heap-allocated data)
- ❌ Hash/Eq require custom implementation for struct variants
- ✅ Pattern matching is exhaustive (compiler-enforced safety)
- ✅ Metadata directly accessible (no wrapper unwrapping needed)
- ✅ Single code path for all types (no Info enum indirection)

### 2. Detection Flow: Pattern First, LLM Creates Custom

**Decision**: Try deterministic pattern matching first. If pattern fails, LLM ALWAYS creates a custom type (even if LLM returns a name matching a known type).

**Alternatives Considered**:
- Map LLM response to known types → Rejected: Complex logic, ambiguous (why did pattern fail but LLM succeeded?)
- Always use LLM → Rejected: Too slow and expensive for common cases
- Parallel execution (pattern + LLM) → Rejected: Wastes LLM calls, no benefit

**Rationale**:
- ✅ **Clear boundary**: Pattern detection = known types, LLM fallback = custom types
- ✅ **Simpler logic**: No `from_name()` mapping, no "is this known?" checks
- ✅ **Debugging clarity**: Can distinguish "Turborepo found by pattern" from "Turborepo found by LLM"
- ✅ **Preserves metadata**: LLM-discovered types always have full metadata (config files, cache dirs)
- ✅ **No false positives**: If pattern matching failed, there was a reason - trust that and use custom type

**Example**:
```rust
// Scenario: Project has turbo.json but with unusual structure
// Pattern matching: FAIL (turbo.json doesn't match expected pattern)
// LLM response: "turborepo" with custom config locations

// OLD (complex):
// - Check if "turborepo" is known → Yes
// - Return OrchestratorId::Turborepo
// - Why did pattern fail? Confusing!

// NEW (simple):
// - Pattern failed → Use LLM
// - Create OrchestratorId::Custom("turborepo") with LLM metadata
// - Clear: This is a non-standard Turborepo setup
```

**Implementation**:
```rust
pub async fn detect_build_system_with_llm(
    &self,
    manifest_path: &Path,
    content: &str,
    llm: &dyn LLMClient,
) -> Result<BuildSystemId> {
    // Fast path: Try pattern-based detection
    if let Some(id) = self.detect_build_system(manifest_path, content) {
        return Ok(id);  // ~5ms, no LLM call
    }

    // Slow path: LLM identification - ALWAYS creates custom type
    let response = llm.identify_build_system(manifest_path, content).await?;  // ~200-500ms

    // Return custom type (ephemeral - not registered in registry)
    Ok(BuildSystemId::Custom(response.name))
}
```

### 3. Custom Type Implementation: Struct + Trait

**Decision**: Implement traits for custom types using LLM-provided metadata.

**Alternatives Considered**:
- Store raw JSON and parse on-demand → Rejected: Poor performance, no type safety
- Generate Rust code dynamically → Rejected: Unnecessary complexity
- Marker traits only (no behavior) → Rejected: Doesn't provide useful functionality

**Rationale**:
- Custom types work exactly like hardcoded ones (uniform interface)
- Trait methods return LLM-discovered data (cache dirs, build commands, etc.)
- No runtime code generation needed

**Example**:
```rust
pub struct CustomBuildSystem {
    pub name: String,
    pub manifest_files: Vec<String>,
    pub build_commands: Vec<String>,
    pub cache_dirs: Vec<String>,
    pub confidence: f32,
}

impl BuildSystem for CustomBuildSystem {
    fn id(&self) -> BuildSystemId {
        BuildSystemId::Custom(self.name.clone())
    }

    fn cache_directories(&self) -> Vec<String> {
        self.cache_dirs.clone()  // Use LLM-provided data
    }

    fn detect(&self, _filename: &str, _content: Option<&str>) -> bool {
        false  // Custom types don't self-detect (only created from LLM)
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

### 6. LLM Prompt Enhancement Strategy

**Decision**: Enhance existing phase prompts to return typed IDs with metadata instead of creating entirely new prompts.

**Current Prompts That Already Return Type Names**:
- Phase 3 (Structure): `"orchestrator": "turborepo" | "nx" | "lerna" | ...`
- Phase 6 (Runtime): `"framework": "nextjs" | "express" | ...`
- Phase 7 (Build): Returns build commands (implicitly tied to BuildSystemId)

**Enhancement Strategy**:
1. **Keep existing prompt structure** (minimal changes to proven prompts)
2. **Add metadata fields** for custom type creation
3. **Post-process responses** to map names → typed IDs

**Example: Phase 3 Orchestrator Enhancement**:
```diff
  // Current prompt (src/pipeline/phases/03_structure.rs:68)
  "orchestrator": "turborepo" | "nx" | "lerna" | "rush" | "bazel" | "pants" | "buck" | "none" | null

+ // Enhanced: Add metadata fields (always present when LLM is used)
+ "orchestrator": {
+   "name": "bazel",                          // Required
+   "config_files": ["BUILD", "WORKSPACE"],   // Optional (defaults to [])
+   "cache_dirs": ["bazel-out"]               // Optional (defaults to [])
+ } | null
+
+ // Note: If deterministic detection succeeds, LLM is not called at all
+ // LLM response ALWAYS creates Custom type, even if name is "turborepo"
```

**Processing Logic** (Simplified - Metadata in Custom Variant):
```rust
// Rule: LLM fallback ALWAYS creates ephemeral custom type
// Metadata stored directly in BuildSystemId/OrchestratorId/etc. Custom variant

pub async fn detect_orchestrator_with_llm(
    scan: &ScanResult,
    classify: &ClassifyResult,
    llm: &dyn LLMClient,
) -> Result<Option<OrchestratorId>> {
    // 1. Try deterministic detection first (pattern matching)
    if let Some(orch_id) = detect_orchestrator_deterministic(scan) {
        return Ok(Some(orch_id));  // Returns known variant (Turborepo, Nx, etc.)
    }

    // 2. Deterministic failed - use LLM (ephemeral response)
    let prompt = build_prompt(scan, classify);
    let response: LLMStructure = llm.chat(prompt).await?;

    // 3. Return custom variant with metadata
    Ok(response.orchestrator.map(|orch| {
        OrchestratorId::Custom {
            name: orch.name,
            config_files: orch.config_files,
            cache_dirs: orch.cache_dirs,
        }
    }))
}
```

**Validation**:
- Required fields: name (when orchestrator detected)
- Optional metadata: config_files, build_commands, cache_dirs, dependency_patterns (default to empty arrays)
- Name must be non-empty string
- All LLM responses create custom types (no mapping to known types)

## Data Flow

```
User Request
    │
    ▼
DetectionService
    │
    ▼
PipelineOrchestrator
    │
    ▼
Phase 1: Scan
    │
    └─ Returns: ScanResult with file list
    │
    ▼
Phase 3: Structure
    │
    ├─ Try Pattern Detection for Orchestrator
    │      ├─ Match found → OrchestratorId::Turborepo
    │      └─ No match → LLM call
    │
    └─ LLM Orchestrator Identification
           ├─ LLMClient.chat(...) → ~200-500ms
           ├─ Response validation (confidence check)
           └─ OrchestratorId::Custom { name, config_files, cache_dirs }
    │
    └─ Returns: StructureResult { orchestrator: Option<OrchestratorId>, ... }
    │
    ▼
Phase 6: Runtime (per service)
    │
    ├─ Try Pattern Detection for Language/BuildSystem
    │      ├─ Match found → BuildSystemId::Cargo
    │      └─ No match → LLM call
    │
    └─ LLM Build System Identification
           ├─ LLMClient.chat(...) → ~200-500ms
           ├─ Response validation
           └─ BuildSystemId::Custom { name, manifest_files, build_commands, cache_dirs }
    │
    └─ Returns: Service { build_system: BuildSystemId, language: LanguageId, ... }
    │
    ▼
Phase 13: Cache
    │
    └─ Match on BuildSystemId::Custom to read cache_dirs or use registry for known variants
    │
    └─ Returns: CacheResult with cache directories

Metadata Storage: Custom variants (BuildSystemId::Custom, OrchestratorId::Custom, etc.)
contain metadata directly - no separate Info enums or global registry needed.
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

## Migration Path

1. Add `Custom { name, metadata... }` struct variants to all ID enums (LanguageId, BuildSystemId, FrameworkId, OrchestratorId)
2. Update `name()` methods to handle Custom variant (return `name` field)
3. Add LLM response schemas for custom type metadata
4. Update Phase 3 (Structure) to return `OrchestratorId::Custom { ... }` on LLM fallback
5. Update Phase 6 (Runtime) to return `BuildSystemId::Custom { ... }`, `LanguageId::Custom { ... }`, etc. on LLM fallback
6. Update downstream phases (Cache, Build, Entrypoint) to match on Custom variants
7. Fix all pattern matches to handle Custom variants (compiler will warn about non-exhaustive matches)
8. Add tests and fixtures for custom types

No breaking changes to external JSON API (Custom variants serialize with name field).

## Future Enhancements

### 1. Learning System
Store successful custom type detections and promote to hardcoded types after confidence threshold (e.g., 10 successful detections).

### 2. User-Provided Definitions
Allow users to provide custom type definitions in config file (bypass LLM for known-internal tech).

### 3. Confidence Weighting
Use confidence scores to rank multiple detection results.

### 4. Type Suggestions
When LLM detects similar custom type, suggest mapping to known type (e.g., "npm-workspaces" → "npm").
