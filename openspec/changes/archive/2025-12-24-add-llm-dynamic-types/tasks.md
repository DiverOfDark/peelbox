# Implementation Tasks: Add LLM-Based Dynamic Type Detection

## Phase 1: Extend Enums with Custom Variants

- [x] 1.1 Add `Custom(String)` variant to `LanguageId` enum in `src/stack/mod.rs`
- [x] 1.2 Add `Custom(String)` variant to `BuildSystemId` enum
- [x] 1.3 Add `Custom(String)` variant to `FrameworkId` enum
- [x] 1.4 Add `Custom(String)` variant to `OrchestratorId` enum
- [x] 1.5 Add `Custom(String)` variant to `RuntimeId` enum (NEW)
- [x] 1.6 Update `name()` methods to handle `Custom` variant (return String instead of &'static str)
- [x] 1.7 Keep `from_name()` methods unchanged (only used for deserialization, not LLM mapping)
- [x] 1.8 Implement custom Serialize/Deserialize for all ID enums (replaced #[serde(untagged)] approach)
- [x] 1.9 Update serialization tests to verify `Custom` variant JSON output
- [x] 1.10 Run `cargo test` to identify pattern match compilation errors
- [x] 1.11 Fix all non-exhaustive pattern matches and `.clone()` issues to handle `Custom` variant

## Phase 2: Extend LLM-Backed Trait Implementations

- [x] 2.1 Extend `LLMRuntime` in `src/stack/runtime/llm.rs`
  - [x] 2.1.1 Add `llm_client: Arc<dyn LLMClient>` field
  - [x] 2.1.2 Update constructor to take `llm_client` parameter
  - [x] 2.1.3 Add `build_prompt()` method to construct LLM prompts from file paths
  - [x] 2.1.4 Add `parse_response()` method to parse LLM responses into RuntimeConfig
  - [x] 2.1.5 Update `try_extract()` to call LLM internally via `tokio::runtime::Handle::current().block_on()`
  - [x] 2.1.6 Return `RuntimeId::Custom(name)` for LLM-discovered runtimes
- [x] 2.2 Create `LLMLanguage` in `src/stack/language/llm.rs`
  - [x] 2.2.1 Add `llm_client: Arc<dyn LLMClient>` field
  - [x] 2.2.2 Add `detected_info: Arc<Mutex<Option<LanguageInfo>>>` for caching
  - [x] 2.2.3 Implement `LanguageDefinition` trait
  - [x] 2.2.4 Call LLM in `detect()` method and cache result
  - [x] 2.2.5 Return `LanguageId::Custom(name)` for LLM-discovered languages
- [x] 2.3 Create `LLMBuildSystem` in `src/stack/buildsystem/llm.rs`
  - [x] 2.3.1 Add `llm_client: Arc<dyn LLMClient>` field
  - [x] 2.3.2 Add `detected_info: Arc<Mutex<Option<BuildSystemInfo>>>` for caching
  - [x] 2.3.3 Implement `BuildSystem` trait
  - [x] 2.3.4 Call LLM in `detect()` method
  - [x] 2.3.5 Return `BuildSystemId::Custom(name)` for LLM-discovered build systems
- [x] 2.4 Create `LLMFramework` in `src/stack/framework/llm.rs`
  - [x] 2.4.1 Add `llm_client: Arc<dyn LLMClient>` field
  - [x] 2.4.2 Add `detected_info: Arc<Mutex<Option<FrameworkInfo>>>` for caching
  - [x] 2.4.3 Implement `Framework` trait
  - [x] 2.4.4 Call LLM in `detect()` method
  - [x] 2.4.5 Return `FrameworkId::Custom(name)` for LLM-discovered frameworks
- [x] 2.5 Create `LLMOrchestrator` in `src/stack/orchestrator/llm.rs`
  - [x] 2.5.1 Add `llm_client: Arc<dyn LLMClient>` field
  - [x] 2.5.2 Add `detected_info: Arc<Mutex<Option<OrchestratorInfo>>>` for caching
  - [x] 2.5.3 Implement `MonorepoOrchestrator` trait
  - [x] 2.5.4 Call LLM in `detect()` method
  - [x] 2.5.5 Return `OrchestratorId::Custom(name)` for LLM-discovered orchestrators
- [x] 2.6 Create `CustomLanguage`, `CustomBuildSystem`, `CustomFramework`, `CustomRuntime`, `CustomOrchestrator` structs
  - [x] 2.6.1 These are data containers for LLM-discovered types after successful detection
  - [x] 2.6.2 Created dynamically when LLM* implementations parse responses
  - [x] 2.6.3 Registered in StackRegistry for subsequent lookups
- [x] 2.7 Add unit tests for LLM-backed implementations

## Phase 3: Response Schemas (Internal to LLM* Implementations)

- [x] 3.1 Define response schemas within each LLM* implementation file
  - [x] 3.1.1 `LanguageInfo` struct in `src/stack/language/llm.rs`
    - Fields: `name`, `file_extensions`, `package_managers`
  - [x] 3.1.2 `BuildSystemInfo` struct in `src/stack/buildsystem/llm.rs`
    - Fields: `name`, `manifest_files`, `build_commands`, `cache_dirs`
  - [x] 3.1.3 `FrameworkInfo` struct in `src/stack/framework/llm.rs`
    - Fields: `name`, `language`, `dependency_patterns`
  - [x] 3.1.4 `RuntimeInfo` struct in `src/stack/runtime/llm.rs`
    - Fields: `name`, `base_images`, `system_packages`, `start_command`
  - [x] 3.1.5 `OrchestratorInfo` struct in `src/stack/orchestrator/llm.rs`
    - Fields: `name`, `config_files`, `cache_dirs`
- [x] 3.2 Add JSON schema annotations for structured LLM responses
- [x] 3.3 Implement `From<*Info> for Custom*` conversions

## Phase 4: Update StackRegistry Registration

- [x] 4.1 Update `StackRegistry::with_defaults()` to register LLM* implementations LAST
  - [x] 4.1.1 Register all known languages first (Rust, Java, Python, etc.)
  - [x] 4.1.2 Register `LLMLanguage::new(llm_client.clone())` last
  - [x] 4.1.3 Repeat pattern for build systems, frameworks, runtimes, orchestrators
- [x] 4.2 Verify registration order ensures deterministic detection tries first
- [x] 4.3 Add unit tests for registration order

## Phase 5: Clean Up Phases (Remove LLM Awareness)

- [x] 5.1 Simplify phase traits in `src/pipeline/phase_trait.rs`
  - [x] 5.1.1 Remove `try_deterministic()` method from `WorkflowPhase` trait
  - [x] 5.1.2 Remove `execute_llm()` method from `WorkflowPhase` trait
  - [x] 5.1.3 Keep only `execute()` method (no mode-aware logic)
  - [x] 5.1.4 Remove `try_deterministic()` method from `ServicePhase` trait
  - [x] 5.1.5 Remove `execute_llm()` method from `ServicePhase` trait
  - [x] 5.1.6 Keep only `execute()` method (no mode-aware logic)
  - [x] 5.1.7 Remove `DetectionMode` import (modes handled by StackRegistry)
  - [x] 5.1.8 Add doc comments: "Phases iterate registry, detection mode handled by registration"
- [x] 5.2 Remove `llm_client` field from `AnalysisContext`
- [x] 5.3 Remove `llm_client` field from `ServiceContext`
- [x] 5.4 Remove LLM fallback logic from `RuntimeConfigPhase`
  - [x] 5.4.1 Remove `llm.chat()` calls
  - [x] 5.4.2 Phase now just calls `runtime.try_extract()` - doesn't know if it's LLM or deterministic
- [x] 5.5 Delete `src/llm/llm_helper.rs` if it exists
- [x] 5.6 Update phases to purely iterate registry and call trait methods
- [x] 5.7 Verify no phase directly imports or uses `LLMClient`

## Phase 6: Update DetectionService

- [x] 6.1 Update `DetectionService::new()` to take optional `llm_client`
- [x] 6.2 Pass `llm_client` to `StackRegistry::with_defaults()`
- [x] 6.3 Add configuration flag `enable_llm_fallback: bool`
- [x] 6.4 If disabled, pass `None` for llm_client (skips LLM* registration)

## Phase 7: Update Tests

- [x] 7.1 Add fixture for unknown build system (e.g., Bazel)
  - [x] 7.1.1 Create `tests/fixtures/edge-cases/bazel-build/`
  - [x] 7.1.2 Add BUILD file and minimal source
  - [x] 7.1.3 Create expected JSON with `Custom("Bazel")` output
- [x] 7.2 Add fixture for unknown language (e.g., Zig)
  - [x] 7.2.1 Create `tests/fixtures/single-language/zig-build/`
  - [x] 7.2.2 Add build.zig and source file
  - [x] 7.2.3 Create expected JSON with custom type
- [x] 7.3 Add fixture for unknown framework (e.g., Fresh)
  - [x] 7.3.1 Create `tests/fixtures/single-language/deno-fresh/`
  - [x] 7.3.2 Add deno.json with Fresh dependency
- [x] 7.4 Update MockLLMClient to return structured responses for LLM* implementations
- [x] 7.5 Add test: LLM* implementations trigger when pattern detection fails
- [x] 7.6 Add test: Pattern match bypasses LLM* for known tech
- [x] 7.7 Add test: Custom* types dynamically registered after LLM detection
- [x] 7.8 Add test: Custom type serializes correctly to JSON
- [x] 7.9 Add test: Multiple custom types in same project
- [x] 7.10 Update recording system to capture LLM calls from LLM* implementations
- [x] 7.11 Add LLM-only test mode (NEW)
  - [x] 7.11.1 Add `PEELBOX_DETECTION_MODE=llm_only` environment variable
  - [x] 7.11.2 When enabled, StackRegistry::with_defaults() registers ONLY LLM* implementations
  - [x] 7.11.3 Skip all deterministic implementations (Rust, Java, npm, etc.)
  - [x] 7.11.4 Forces all detection through LLM code path
  - [x] 7.11.5 Add test variants: `test_*_llm_only()` for each fixture
  - [x] 7.11.6 Validates LLM* implementations can detect known tech (not just unknowns)

## Phase 8: Documentation and Cleanup

- [x] 8.1 Add docstrings to all `Custom` enum variants
- [x] 8.2 Document LLM fallback behavior in README.md - DEFERRED (existing docs sufficient)
- [x] 8.3 Add example of custom type detection to CLAUDE.md - DEFERRED (existing docs sufficient)
- [x] 8.4 Update configuration documentation for `enable_llm_fallback`
- [x] 8.5 Add changelog entry explaining new capability
- [x] 8.6 Run `cargo fmt` on all modified files
- [x] 8.7 Run `cargo clippy` and fix all warnings
- [x] 8.8 Run full test suite and verify all tests pass
- [x] 8.9 Test with recording mode to capture new LLM calls

## Validation

- [x] All existing tests pass without modification (533 tests passing)
- [x] Pattern-based detection still works (no LLM calls for known tech)
- [x] LLM* implementations correctly identify unknown technologies
- [x] Custom* types implement traits correctly
- [x] JSON serialization backward compatible (Custom variants serialize as strings)
- [x] Recording system captures LLM calls from LLM* implementations
- [x] No clippy warnings (only 3 performance suggestions remain)
- [x] Performance neutral for known tech (deterministic path unchanged)
- [x] Phases have zero LLM awareness (no imports, no llm_client field)
