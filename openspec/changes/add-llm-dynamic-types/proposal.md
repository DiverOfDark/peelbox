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
```

### 2. Implement Custom Type Providers

Create dynamic trait implementations that wrap LLM responses:

```rust
/// LLM-discovered language with runtime-provided data
pub struct CustomLanguage {
    pub name: String,
    pub file_extensions: Vec<String>,
    pub package_managers: Vec<String>,
    pub confidence: f32,
    pub reasoning: String,
}

impl LanguageDefinition for CustomLanguage {
    fn id(&self) -> LanguageId {
        LanguageId::Custom(self.name.clone())
    }

    fn file_extensions(&self) -> &[&str] {
        // Convert Vec<String> to &[&str] using temporary storage
    }

    fn name(&self) -> &str {
        &self.name
    }

    // Other trait methods implemented from LLM-provided data
}

/// LLM-discovered build system
pub struct CustomBuildSystem {
    pub name: String,
    pub manifest_files: Vec<String>,
    pub build_commands: Vec<String>,
    pub cache_dirs: Vec<String>,
    pub confidence: f32,
}

/// LLM-discovered framework
pub struct CustomFramework {
    pub name: String,
    pub language: String,  // Which language it's for
    pub dependency_patterns: Vec<String>,
    pub confidence: f32,
}

/// LLM-discovered orchestrator
pub struct CustomOrchestrator {
    pub name: String,
    pub config_files: Vec<String>,
    pub cache_dirs: Vec<String>,
}
```

### 3. Add LLM Fallback Detection (Simplified)

Extend StackRegistry with LLM-based detection as fallback. **Key rule: LLM path ALWAYS creates custom types, no mapping.**

```rust
impl StackRegistry {
    /// Try hardcoded detection first, fallback to LLM on failure
    /// LLM responses ALWAYS create custom types (no mapping to known types)
    pub async fn detect_build_system_with_llm(
        &self,
        manifest_path: &Path,
        content: &str,
        llm: &dyn LLMClient,
    ) -> Result<BuildSystemId> {
        // 1. Try pattern-based detection (existing logic)
        if let Some(id) = self.detect_build_system(manifest_path, content) {
            return Ok(id);  // Known type - return immediately
        }

        // 2. Pattern failed - use LLM and ALWAYS create custom type
        let response = llm.identify_build_system(manifest_path, content).await?;

        // 3. Create custom build system (even if name is "cargo", "npm", etc.)
        let custom = Arc::new(CustomBuildSystem {
            name: response.name.clone(),
            manifest_files: response.manifest_files,
            build_commands: response.build_commands,
            cache_dirs: response.cache_dirs,
            confidence: response.confidence,
        });

        let id = BuildSystemId::Custom(response.name);
        self.register_build_system_runtime(custom);
        Ok(id)
    }
}
```

**Rationale**: No `from_name()` mapping = simpler logic, clear boundary (deterministic = known, LLM = custom)

### 4. Add Runtime Registration

Support dynamic registration of LLM-discovered types:

```rust
impl StackRegistry {
    /// Register a custom language discovered by LLM
    pub fn register_language_runtime(
        &mut self,
        id: LanguageId,
        language: Arc<dyn LanguageDefinition>,
    ) {
        match &id {
            LanguageId::Custom(name) => {
                self.languages.insert(id, language);
            },
            _ => {
                // Known types should use register_language()
                panic!("Use register_language() for known types");
            }
        }
    }

    // Similar for build_system, framework, orchestrator
}
```

### 5. Update Detection Flow

Modify detection to try patterns first, LLM as fallback:

```rust
// In PipelineOrchestrator or DetectionService
pub async fn detect_stack(
    &self,
    repo_path: &Path,
) -> Result<Vec<DetectionStack>> {
    let registry = &self.stack_registry;

    // 1. Scan files
    let manifests = scan_manifests(repo_path, registry)?;

    for manifest in manifests {
        let content = fs::read_to_string(&manifest)?;

        // 2. Try pattern-based detection (fast path)
        if let Some(stack) = registry.detect_stack(&manifest, &content) {
            results.push(stack);
            continue;
        }

        // 3. Fallback to LLM (slow path for unknown tech)
        let build_system = registry
            .detect_build_system_with_llm(&manifest, &content, &self.llm)
            .await?;

        let language = registry
            .detect_language_with_llm(&manifest, &content, build_system, &self.llm)
            .await?;

        results.push(DetectionStack::new(build_system, language, manifest));
    }
}
```

## Impact

### Affected Specs

- **stack-registry** (new): Type system with Custom variants and LLM fallback
- **prompt-pipeline**: Phase 1 (Scan) and Phase 6 (Runtime) updated to use LLM fallback

### Affected Code

**New files**:
- `src/stack/custom/mod.rs` - Module for custom types (~300 lines)
- `src/stack/custom/language.rs` - CustomLanguage implementation (~100 lines)
- `src/stack/custom/buildsystem.rs` - CustomBuildSystem implementation (~100 lines)
- `src/stack/custom/framework.rs` - CustomFramework implementation (~100 lines)
- `src/stack/custom/orchestrator.rs` - CustomOrchestrator implementation (~50 lines)

**Modified files**:
- `src/stack/mod.rs` - Add `Custom(String)` to all ID enums (~50 lines changed)
- `src/stack/registry.rs` - Add `detect_*_with_llm()` and `register_*_runtime()` methods (~200 lines added)
- `src/pipeline/orchestrator.rs` - Use LLM fallback in detection flow (~50 lines changed)
- `src/detection/service.rs` - Pass LLM client to registry (~20 lines changed)

**LLM prompts** (new):
- `identify_language` - JSON schema for language detection
- `identify_build_system` - JSON schema for build system detection
- `identify_framework` - JSON schema for framework detection
- `identify_orchestrator` - JSON schema for orchestrator detection

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

1. Add `Custom(String)` variants to all enums in `src/stack/mod.rs`
2. Implement `CustomLanguage`, `CustomBuildSystem`, `CustomFramework`, `CustomOrchestrator` structs
3. Add `detect_*_with_llm()` methods to StackRegistry (async)
4. Update PipelineOrchestrator to use LLM fallback
5. Add LLM prompt schemas for type identification
6. Update all pattern matches to handle `Custom` variant
7. Add tests for custom type detection
8. Update recording system to capture LLM type identification calls

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

## Success Criteria

✅ All existing tests pass with `Custom` variants added
✅ Pattern-based detection still works without LLM calls
✅ LLM fallback correctly identifies unknown build systems
✅ CustomLanguage/CustomBuildSystem implement traits correctly
✅ Runtime registration works for LLM-discovered types
✅ JSON serialization backward compatible
✅ All pattern matches handle `Custom` variant
✅ Recording system captures custom type LLM calls
✅ Performance neutral for known tech (no regression)
✅ Documentation explains when LLM fallback triggers
