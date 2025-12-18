# Implementation Tasks: Add LLM-Based Dynamic Type Detection

## Phase 1: Extend Enums with Custom Variants

- [ ] 1.1 Add `Custom(String)` variant to `LanguageId` enum in `src/stack/mod.rs`
- [ ] 1.2 Add `Custom(String)` variant to `BuildSystemId` enum
- [ ] 1.3 Add `Custom(String)` variant to `FrameworkId` enum
- [ ] 1.4 Add `Custom(String)` variant to `OrchestratorId` enum
- [ ] 1.5 Update `name()` methods to handle `Custom` variant (return inner String)
- [ ] 1.6 Keep `from_name()` methods unchanged (only used for deserialization, not LLM mapping)
- [ ] 1.7 Add `#[serde(untagged)]` attribute to all ID enums for backward-compatible JSON
- [ ] 1.8 Update serialization tests to verify `Custom` variant JSON output
- [ ] 1.9 Run `cargo test` to identify pattern match compilation errors
- [ ] 1.10 Fix all non-exhaustive pattern matches to handle `Custom` variant

## Phase 2: Create Custom Type Structs

- [ ] 2.1 Create `src/stack/custom/mod.rs` with module declaration
- [ ] 2.2 Add `pub mod custom;` to `src/stack/mod.rs`
- [ ] 2.3 Create `src/stack/custom/language.rs` with `CustomLanguage` struct
  - [ ] 2.3.1 Add fields: `name`, `file_extensions`, `package_managers`, `confidence`, `reasoning`
  - [ ] 2.3.2 Implement `LanguageDefinition` trait for `CustomLanguage`
  - [ ] 2.3.3 Implement `id()` method returning `LanguageId::Custom(name)`
  - [ ] 2.3.4 Implement `file_extensions()` using cached slice conversion
  - [ ] 2.3.5 Implement `detect()` method (always returns None - custom types don't self-detect)
- [ ] 2.4 Create `src/stack/custom/buildsystem.rs` with `CustomBuildSystem` struct
  - [ ] 2.4.1 Add fields: `name`, `manifest_files`, `build_commands`, `cache_dirs`, `confidence`
  - [ ] 2.4.2 Implement `BuildSystem` trait for `CustomBuildSystem`
  - [ ] 2.4.3 Implement `id()` method returning `BuildSystemId::Custom(name)`
  - [ ] 2.4.4 Implement `detect()` method (returns None)
  - [ ] 2.4.5 Implement `cache_directories()` from LLM-provided data
- [ ] 2.5 Create `src/stack/custom/framework.rs` with `CustomFramework` struct
  - [ ] 2.5.1 Add fields: `name`, `language`, `dependency_patterns`, `confidence`
  - [ ] 2.5.2 Implement `Framework` trait for `CustomFramework`
  - [ ] 2.5.3 Implement `id()` method returning `FrameworkId::Custom(name)`
  - [ ] 2.5.4 Implement `detect()` method (returns None)
  - [ ] 2.5.5 Parse `language` string to LanguageId (known or custom)
- [ ] 2.6 Create `src/stack/custom/orchestrator.rs` with `CustomOrchestrator` struct
  - [ ] 2.6.1 Add fields: `name`, `config_files`, `cache_dirs`
  - [ ] 2.6.2 Implement `MonorepoOrchestrator` trait for `CustomOrchestrator`
  - [ ] 2.6.3 Implement `id()` method returning `OrchestratorId::Custom(name)`
- [ ] 2.7 Add unit tests for all custom type implementations

## Phase 3: Add LLM Identification Schemas

- [ ] 3.1 Create `src/llm/schemas/mod.rs` for LLM response schemas
- [ ] 3.2 Define `LanguageIdentification` struct with serde
  - [ ] 3.2.1 Fields: `name`, `file_extensions`, `package_managers`, `confidence`, `reasoning`
  - [ ] 3.2.2 Add JSON schema annotations for LLM function calling
- [ ] 3.3 Define `BuildSystemIdentification` struct
  - [ ] 3.3.1 Fields: `name`, `manifest_files`, `build_commands`, `cache_dirs`, `confidence`
- [ ] 3.4 Define `FrameworkIdentification` struct
  - [ ] 3.4.1 Fields: `name`, `language`, `dependency_patterns`, `confidence`
- [ ] 3.5 Define `OrchestratorIdentification` struct
  - [ ] 3.5.1 Fields: `name`, `config_files`, `cache_dirs`
- [ ] 3.6 Create conversion methods: `IdentificationSchema -> CustomType`

## Phase 4: Add LLM Client Methods

- [ ] 4.1 Add `identify_build_system()` to `LLMClient` trait in `src/llm/client.rs`
  - [ ] 4.1.1 Takes `manifest_path: &Path`, `content: &str`
  - [ ] 4.1.2 Returns `Result<BuildSystemIdentification>`
  - [ ] 4.1.3 Prompt: Analyze manifest and identify build system with metadata
- [ ] 4.2 Add `identify_language()` to `LLMClient` trait
  - [ ] 4.2.1 Takes `manifest_path: &Path`, `content: &str`, `build_system: &str`
  - [ ] 4.2.2 Returns `Result<LanguageIdentification>`
- [ ] 4.3 Add `identify_framework()` to `LLMClient` trait
  - [ ] 4.3.1 Takes `dependencies: &[String]`, `language: &str`
  - [ ] 4.3.2 Returns `Result<Vec<FrameworkIdentification>>`
- [ ] 4.4 Add `identify_orchestrator()` to `LLMClient` trait
  - [ ] 4.4.1 Takes `config_files: &[(PathBuf, String)]`
  - [ ] 4.4.2 Returns `Result<Option<OrchestratorIdentification>>`
- [ ] 4.5 Implement methods in `GenAIClient` (src/llm/genai.rs)
- [ ] 4.6 Implement methods in `MockLLMClient` (return predefined responses)
- [ ] 4.7 Add recording support for new methods in `RecordingClient`

## Phase 5: Extend StackRegistry with LLM Fallback

- [ ] 5.1 Add `detect_build_system_with_llm()` to StackRegistry
  - [ ] 5.1.1 Try `detect_build_system()` first (pattern-based)
  - [ ] 5.1.2 Return immediately if pattern match succeeds
  - [ ] 5.1.3 Call `llm.identify_build_system()` if pattern fails
  - [ ] 5.1.4 Create CustomBuildSystem from LLM response (no mapping to known types)
  - [ ] 5.1.5 Create `CustomBuildSystem` if unknown
  - [ ] 5.1.6 Register custom build system in registry
  - [ ] 5.1.7 Return `BuildSystemId::Custom(name)`
- [ ] 5.2 Add `detect_language_with_llm()` to StackRegistry
  - [ ] 5.2.1 Try `detect_language()` first
  - [ ] 5.2.2 Fallback to LLM if pattern fails
  - [ ] 5.2.3 Register custom language if unknown
- [ ] 5.3 Add `detect_framework_with_llm()` to StackRegistry
  - [ ] 5.3.1 Try deterministic framework detection first
  - [ ] 5.3.2 Fallback to LLM with dependency list
  - [ ] 5.3.3 Register custom frameworks
- [ ] 5.4 Add `detect_orchestrator_with_llm()` to StackRegistry
  - [ ] 5.4.1 Try pattern-based orchestrator detection
  - [ ] 5.4.2 Fallback to LLM with config files
- [ ] 5.5 Add `register_language_runtime()` for dynamic registration
- [ ] 5.6 Add `register_build_system_runtime()` for dynamic registration
- [ ] 5.7 Add `register_framework_runtime()` for dynamic registration
- [ ] 5.8 Add `register_orchestrator_runtime()` for dynamic registration
- [ ] 5.9 Add unit tests for LLM fallback logic

## Phase 6: Update Detection Flow

- [ ] 6.1 Update `PipelineOrchestrator::run()` to use LLM fallback
  - [ ] 6.1.1 Pass LLM client to registry methods
  - [ ] 6.1.2 Use `detect_build_system_with_llm()` in Phase 1 (Scan)
  - [ ] 6.1.3 Use `detect_language_with_llm()` in Phase 6 (Runtime)
  - [ ] 6.1.4 Use `detect_framework_with_llm()` in Phase 6 (Runtime)
  - [ ] 6.1.5 Use `detect_orchestrator_with_llm()` in Phase 3 (Structure)
- [ ] 6.2 Update `DetectionService::detect()` to pass LLM to pipeline
- [ ] 6.3 Add configuration flag `enable_llm_fallback: bool` to DetectionService
- [ ] 6.4 Skip LLM calls if flag is disabled (fail fast on unknown)

## Phase 7: Update Tests

- [ ] 7.1 Add fixture for unknown build system (e.g., Bazel)
  - [ ] 7.1.1 Create `tests/fixtures/edge-cases/bazel-build/`
  - [ ] 7.1.2 Add BUILD file and minimal source
  - [ ] 7.1.3 Create expected JSON with `Custom("Bazel")` output
- [ ] 7.2 Add fixture for unknown language (e.g., Zig)
  - [ ] 7.2.1 Create `tests/fixtures/single-language/zig-build/`
  - [ ] 7.2.2 Add build.zig and source file
  - [ ] 7.2.3 Create expected JSON with custom type
- [ ] 7.3 Add fixture for unknown framework (e.g., Fresh)
  - [ ] 7.3.1 Create `tests/fixtures/single-language/deno-fresh/`
  - [ ] 7.3.2 Add deno.json with Fresh dependency
- [ ] 7.4 Update MockLLMClient to return identification responses
- [ ] 7.5 Add test: LLM fallback triggers when pattern fails
- [ ] 7.6 Add test: Pattern match bypasses LLM for known tech
- [ ] 7.7 Add test: Custom type registered in registry
- [ ] 7.8 Add test: Custom type serializes correctly to JSON
- [ ] 7.9 Add test: Multiple custom types in same project
- [ ] 7.10 Update recording system tests to handle identification calls

## Phase 8: Documentation and Cleanup

- [ ] 8.1 Add docstrings to all `Custom` enum variants
- [ ] 8.2 Document LLM fallback behavior in README.md
- [ ] 8.3 Add example of custom type detection to CLAUDE.md
- [ ] 8.4 Update configuration documentation for `enable_llm_fallback`
- [ ] 8.5 Add changelog entry explaining new capability
- [ ] 8.6 Run `cargo fmt` on all modified files
- [ ] 8.7 Run `cargo clippy` and fix all warnings
- [ ] 8.8 Run full test suite and verify all tests pass
- [ ] 8.9 Test with recording mode to capture new LLM calls

## Validation

- [ ] All existing tests pass without modification
- [ ] Pattern-based detection still works (no LLM calls for known tech)
- [ ] LLM fallback correctly identifies unknown technologies
- [ ] Custom types implement traits correctly
- [ ] JSON serialization backward compatible
- [ ] Recording system captures custom type identification
- [ ] No clippy warnings
- [ ] Performance neutral for known tech
