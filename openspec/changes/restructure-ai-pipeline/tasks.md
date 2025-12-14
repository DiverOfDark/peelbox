# Tasks: Restructure AI Analysis Pipeline

Each phase delivers compilable, working code that integrates with the existing system.
Old code is removed incrementally as new code takes over its responsibilities.

---

## Phase 1: Language Registry Foundation

Create the language registry and migrate one language (Rust) to prove the pattern.
Existing detection continues to work; new registry runs in parallel for validation.

- [x] 1.1 Create `src/languages/mod.rs` with `LanguageDefinition` trait
- [x] 1.2 Create `src/languages/registry.rs` with `LanguageRegistry`
- [x] 1.3 Implement `src/languages/rust.rs` (extensions, manifests, detection, templates)
- [x] 1.4 Add unit tests for Rust language definition
- [x] 1.5 Wire `LanguageRegistry` into `DetectionService` (unused, just instantiated)
- [x] 1.6 Verify: `cargo build && cargo test` passes

**Deliverable:** New module compiles, existing behavior unchanged.

---

## Phase 2: Migrate Languages to Registry

Add remaining languages. Still not used in production path, but fully tested.

- [x] 2.1 Implement `src/languages/java.rs` (Maven, Gradle)
- [x] 2.2 Implement `src/languages/kotlin.rs`
- [x] 2.3 Implement `src/languages/javascript.rs` (npm, yarn, pnpm, bun)
- [x] 2.4 Implement `src/languages/typescript.rs`
- [x] 2.5 Implement `src/languages/python.rs` (pip, poetry, pipenv)
- [x] 2.6 Implement `src/languages/go.rs`
- [x] 2.7 Implement `src/languages/dotnet.rs`
- [x] 2.8 Implement `src/languages/ruby.rs`
- [x] 2.9 Implement `src/languages/php.rs`
- [x] 2.10 Implement `src/languages/cpp.rs`
- [x] 2.11 Implement `src/languages/elixir.rs`
- [x] 2.12 Add integration test: registry detects same as current code for test repos
- [x] 2.13 Verify: `cargo build && cargo test` passes

**Deliverable:** Full language registry, tested against current detection.

---

## Phase 3: FileSystem Abstraction

Create FileSystem trait and RealFileSystem. Wiring deferred to Phase 11 (Tool System Refactor).

- [x] 3.1 Create `src/fs/mod.rs` module structure
- [x] 3.2 Define `FileSystem` trait in `src/fs/trait.rs`
- [x] 3.3 Implement `RealFileSystem` in `src/fs/real.rs`
- [x] 3.4 ~~Update existing `ToolExecutor` to accept `&dyn FileSystem` parameter~~ (deferred to Phase 11)
- [x] 3.5 ~~Pass `RealFileSystem` from current call sites~~ (deferred to Phase 11)
- [x] 3.6 Add unit tests for `RealFileSystem`
- [x] 3.7 Verify: `cargo build && cargo test` passes

**Deliverable:** FileSystem abstraction ready for Phase 11 integration.

---

## Phase 4: MockFileSystem for Testing

Add MockFileSystem and use it to test existing tools. Tool testing deferred to Phase 11.

- [x] 4.1 Implement `MockFileSystem` in `src/fs/mock.rs`
- [x] 4.2 ~~Add unit tests for each existing tool using `MockFileSystem`~~ (deferred to Phase 11)
- [x] 4.3 Verify: `cargo test` passes

**Deliverable:** MockFileSystem ready for Phase 11 integration.

---

## Phase 5: Bootstrap Scanner

Create bootstrap scanner using LanguageRegistry. Inject into system prompt.

- [x] 5.1 Create `src/bootstrap/mod.rs` module structure
- [x] 5.2 Define `BootstrapContext`, `RepoSummary`, `LanguageDetection` in `src/bootstrap/context.rs`
- [x] 5.3 Implement `BootstrapScanner` using `LanguageRegistry` in `src/bootstrap/scanner.rs`
- [x] 5.4 Implement `format_for_prompt()` for system prompt injection
- [x] 5.5 Add bootstrap context to system prompt in existing `GenAIBackend`
- [x] 5.6 Remove `src/detection/jumpstart/` (replaced by bootstrap)
- [x] 5.7 Add unit tests for bootstrap scanner
- [x] 5.8 Verify: `cargo build && cargo test` passes, detection works with bootstrap

**Deliverable:** Bootstrap pre-scan runs before LLM, enriches system prompt. Old jumpstart removed.

---

## Phase 6: Progress Reporting

Add progress handler trait and wire into existing detection flow.

- [x] 6.1 Create `src/progress/mod.rs` module structure
- [x] 6.2 Define `ProgressHandler` trait in `src/progress/handler.rs`
- [x] 6.3 Implement `NoOpHandler` (default)
- [x] 6.4 Implement `LoggingHandler` in `src/progress/logging.rs`
- [x] 6.5 Add `progress: Option<Arc<dyn ProgressHandler>>` to `GenAIBackend::detect()`
- [x] 6.6 Emit progress events from existing detection loop
- [x] 6.7 Wire `--verbose` CLI flag to use `LoggingHandler`
- [x] 6.8 Verify: `cargo build && cargo test` passes, `--verbose` shows progress

**Deliverable:** Progress events emitted during detection, visible with --verbose.

---

## Phase 7: LLM Client Abstraction

Extract LLM communication into trait. Existing GenAI logic becomes `GenAIClient`.

- [x] 7.1 Create `src/llm/mod.rs` module structure
- [x] 7.2 Define `LLMClient` trait in `src/llm/client.rs`
- [x] 7.3 Define `LLMResponse`, `ToolCall` types in `src/llm/types.rs`
- [x] 7.4 Extract existing genai logic into `GenAIClient` implementing `LLMClient`
- [x] 7.5 Update `GenAIBackend` to use `GenAIClient` via trait (Provider methods made public)
- [x] 7.6 Add unit tests for `GenAIClient`
- [x] 7.7 Verify: `cargo build && cargo test` passes, detection unchanged

**Deliverable:** LLM communication behind trait, GenAI is one implementation.

---

## Phase 8: Mock LLM Client

Add MockLLMClient for testing detection logic without real LLM.

- [x] 8.1 Implement `MockLLMClient` with scripted responses in `src/llm/mock.rs`
- [x] 8.2 Add integration tests using `MockLLMClient` + `MockFileSystem`
- [x] 8.3 Test full detection flow with mocked dependencies
- [x] 8.4 Verify: `cargo test` passes with mock-based integration tests

**Deliverable:** Full detection testable without external dependencies.

---

## Phase 9: Embedded LLM Client

Implement zero-config local inference using Candle. Falls back when no API keys or Ollama available.

- [x] 9.1 Add Candle dependencies to `Cargo.toml` with feature flags (`embedded-llm`, `cuda`, `metal`)
- [x] 9.2 Create `src/llm/embedded/mod.rs` module structure
- [x] 9.3 Implement `HardwareDetector` in `src/llm/embedded/hardware.rs` (RAM, CUDA, Metal detection)
- [x] 9.4 Implement `ModelSelector` in `src/llm/embedded/models.rs` (select model by available RAM)
- [x] 9.5 Implement `ModelDownloader` in `src/llm/embedded/download.rs` (HuggingFace hub integration)
- [x] 9.6 Implement interactive download prompt (skip in CI, detect via `std::io::stdin().is_terminal()`)
- [x] 9.7 Implement `EmbeddedClient` in `src/llm/embedded/client.rs` implementing `LLMClient`
- [x] 9.8 Add `select_llm_client()` function with provider fallback chain (env → Ollama → embedded)
- [x] 9.9 Wire `select_llm_client()` into CLI default behavior
- [x] 9.10 Add unit tests for hardware detection and model selection
- [x] 9.11 Add integration test with small model (1.5B) for CI
- [x] 9.12 Verify: `cargo build && cargo test` passes, `aipack detect` works without any config

**Deliverable:** Zero-config local inference works out of the box.

**Note:** Building with `--features embedded-llm` requires a C++ compiler for tokenizers native dependencies.

---

## Phase 10: Validation System

Extract validation logic into dedicated module. Wire into existing flow.

- [x] 10.1 Create `src/validation/mod.rs` module structure
- [x] 10.2 Define `ValidationRule` trait in `src/validation/rules.rs`
- [x] 10.3 Implement validation rules: `RequiredFieldsRule`, `NonEmptyCommandsRule`, `ValidImageNameRule`, `ConfidenceRangeRule`
- [x] 10.4 Implement `Validator` in `src/validation/validator.rs`
- [x] 10.5 Replace inline validation in `UniversalBuild::validate()` with `Validator`
- [x] 10.6 Add unit tests for each validation rule
- [x] 10.7 Verify: `cargo build && cargo test` passes, validation works

**Deliverable:** Validation extracted, rules are testable individually.

---

## Phase 11: Tool System Refactor

Restructure tools with proper trait, registry, and caching.

- [x] 11.1 Create `src/tools/mod.rs` module structure
- [x] 11.2 Define `Tool` trait in `src/tools/trait_def.rs`
- [x] 11.3 Migrate existing tools to implement `Tool` trait
- [x] 11.4 Implement `ToolRegistry` in `src/tools/registry.rs`
- [x] 11.5 Implement `ToolCache` in `src/tools/cache.rs`
- [x] 11.6 Implement `ToolSystem` facade in `src/tools/system.rs`
- [x] 11.7 Update `GenAIBackend` to use `ToolSystem`
- [x] 11.8 Remove `src/detection/tools/` (replaced by `src/tools/`)
- [x] 11.9 Verify: `cargo build && cargo test` passes, tools work

**Deliverable:** Tools behind clean abstraction with caching. Old tool code removed.

---

## Phase 12: GetBestPractices via Registry

Update best practices tool to use LanguageRegistry instead of hardcoded templates.

- [x] 12.1 Update `GetBestPracticesTool` to accept `&LanguageRegistry`
- [x] 12.2 Implement `best_practices()` lookup via registry
- [x] 12.3 Remove old hardcoded template code
- [x] 12.4 Add tests for best practices with various languages
- [x] 12.5 Verify: `cargo build && cargo test` passes

**Deliverable:** Best practices served from language definitions.

---

## Phase 13: Pipeline Context

Create PipelineContext to own long-lived dependencies.

- [x] 13.1 Create `src/pipeline/mod.rs` module structure
- [x] 13.2 Define `PipelineConfig` in `src/pipeline/config.rs`
- [x] 13.3 Implement `PipelineContext` owning LLMClient, FileSystem, LanguageRegistry, Validator
- [x] 13.4 Update `DetectionService` to own `PipelineContext`
- [x] 13.5 Add `PipelineContext::with_mocks()` for testing
- [x] 13.6 Verify: `cargo build && cargo test` passes

**Deliverable:** Dependencies centralized in context, easy to inject mocks.

---

## Phase 14: Analysis Pipeline

Extract detection loop into AnalysisPipeline. Replace GenAIBackend internals.

- [x] 14.1 Implement `AnalysisPipeline` in `src/pipeline/analysis.rs`
- [x] 14.2 Move detection loop logic from `GenAIBackend` to `AnalysisPipeline`
- [x] 14.3 Integrate bootstrap, progress, tools, validation in pipeline
- [x] 14.4 Update `DetectionService::detect()` to use `AnalysisPipeline`
- [x] 14.5 Remove `GenAIBackend::detect()` method (now in pipeline)
- [x] 14.6 Remove `src/detection/prompt.rs` (replaced by bootstrap)
- [x] 14.7 Clean up `src/detection/mod.rs` exports
- [x] 14.8 Remove unused types from `src/ai/`
- [x] 14.9 Add integration tests for `AnalysisPipeline`
- [x] 14.10 Run `cargo fmt && cargo clippy`
- [x] 14.11 Verify: `cargo build && cargo test && cargo clippy` passes, CLI works

**Deliverable:** Clean pipeline orchestration, old monolithic code removed.

---

## Phase 15: Test Fixtures

Create minimal test repositories for E2E testing.

- [x] 15.1 Create `tests/fixtures/` directory structure
- [x] 15.2 Create single-language fixtures: rust-cargo, rust-workspace
- [x] 15.3 Create single-language fixtures: node-npm, node-yarn, node-pnpm
- [x] 15.4 Create single-language fixtures: python-pip, python-poetry
- [x] 15.5 Create single-language fixtures: java-maven, java-gradle, kotlin-gradle
- [x] 15.6 Create single-language fixtures: go-mod, dotnet-csproj
- [x] 15.7 Create monorepo fixtures: npm-workspaces, turborepo
- [x] 15.8 Create monorepo fixtures: cargo-workspace, gradle-multiproject
- [x] 15.9 Create monorepo fixtures: maven-multimodule, polyglot
- [x] 15.10 Create edge-case fixtures: empty-repo, no-manifest, multiple-manifests
- [x] 15.11 Create edge-case fixtures: nested-projects, vendor-heavy
- [x] 15.12 Create `tests/fixtures/expected/` with expected JSON outputs
- [x] 15.13 Verify: all fixtures are minimal but representative

**Deliverable:** Comprehensive test fixture library covering common and edge cases.

---

## Phase 16: LLM Recording System

Implement request-response recording for deterministic testing.

- [x] 16.1 Create `src/llm/recording.rs` module
- [x] 16.2 Define `RecordingMode` enum (Record, Replay, Auto)
- [x] 16.3 Define `RecordedExchange` and `RecordedRequest` types
- [x] 16.4 Implement `RecordingLLMClient` wrapping any `LLMClient`
- [x] 16.5 Implement request hashing (canonical JSON → MD5)
- [x] 16.6 Implement recording file I/O (JSON format)
- [x] 16.7 Add `AIPACK_RECORDING_MODE` environment variable support
- [x] 16.8 Add `AIPACK_RECORDINGS_DIR` environment variable support
- [x] 16.9 Add unit tests for recording/replay logic
- [x] 16.10 Verify: `cargo build && cargo test` passes

**Deliverable:** LLM responses can be recorded and replayed deterministically.

---

## Phase 17: E2E Test Suite

Implement end-to-end tests using fixtures and recordings.

- [x] 17.1 Create `tests/e2e.rs` test module
- [x] 17.2 Implement test utilities: `create_detection_service()`, `fixture_path()`, `assert_detection()`, `load_expected()`
- [x] 17.3 Implement single-language E2E tests (rust, node, python, java, go)
- [x] 17.4 Implement monorepo E2E tests (npm-workspaces, cargo-workspace)
- [x] 17.5 Implement edge-case E2E tests (empty-repo, no-manifest)
- [x] 17.6 Add performance test: detection timeout validation
- [x] 17.7 Create expected UniversalBuild JSON files in `tests/fixtures/expected/`
- [x] 17.8 Integrate expected JSON validation in `assert_detection()`
- [x] 17.9 Verify recording infrastructure works (RecordingLLMClient integrated)
- [x] 17.10 Verify: `cargo build --tests` compiles successfully

**Deliverable:** Full E2E test coverage with expected output validation and recording infrastructure. Tests validate against golden JSON files and use RecordingMode::Auto for LLM replay. Each test fixture has a corresponding expected output in `tests/fixtures/expected/` that validates language, build system, base images, and core structure.

---

## Phase 18: Monorepo Support

Update detection to return `Vec<UniversalBuild>` for multi-project repositories.

- [x] 18.1 Update `DetectionService::detect()` to return `Vec<UniversalBuild>`
- [x] 18.2 Update `AnalysisPipeline::analyze()` to return `Vec<UniversalBuild>`
- [x] 18.3 Update `submit_detection` tool to accept both single and array formats
- [ ] 18.4 Update `LanguageRegistry` with monorepo detection signals (deferred)
- [ ] 18.5 Add monorepo indicators to `BootstrapContext` (deferred)
- [x] 18.6 Update system prompt to guide LLM on monorepo analysis
- [x] 18.7 Update validation rules for array of UniversalBuild
- [x] 18.8 Update CLI output formatting to handle multiple builds
- [x] 18.9 Update E2E tests to validate `Vec<UniversalBuild>` responses
- [x] 18.10 Verify: `cargo build && cargo test --lib` passes (300 tests passed)

**Deliverable:** aipack returns `Vec<UniversalBuild>` - single element for single-project repos, multiple elements for monorepos. Each runnable application gets its own UniversalBuild entry. LLM can submit either single object or array, pipeline handles both formats transparently.

---

## Phase 19: CI Test Pipeline

Set up GitHub Actions for automated testing.

- [ ] 19.1 Create `.github/workflows/test.yml`
- [ ] 19.2 Configure unit test job (`cargo test --lib`)
- [ ] 19.3 Configure integration test job with replay mode
- [ ] 19.4 Configure E2E recording job (main branch only)
- [ ] 19.5 Add recording auto-commit on main branch
- [ ] 19.6 Add clippy and fmt checks
- [ ] 19.7 Add cargo doc build check
- [ ] 19.8 Verify: CI pipeline runs successfully

**Deliverable:** Automated CI pipeline with recording updates.

---

## Phase 20: Documentation

Update documentation to reflect new architecture.

- [ ] 20.1 Update CLAUDE.md with new module structure
- [ ] 20.2 Add rustdoc comments to public APIs
- [ ] 20.3 Update CHANGELOG.md with refactoring notes
- [ ] 20.4 Document test fixture creation process
- [ ] 20.5 Document recording system usage
- [ ] 20.6 Verify: `cargo doc` generates without warnings

**Deliverable:** Documentation matches implementation.
