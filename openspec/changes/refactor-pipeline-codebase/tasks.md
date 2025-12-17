# Implementation Tasks

## Phase A: Quick Wins (7.5-11.5 hours)

### 1. Create Test Fixtures for Untested Languages (1-2 hours) ✅
- [x] 1.1 Create `tests/fixtures/single-language/ruby-bundler/` with Gemfile + minimal .rb file
- [x] 1.2 Create `tests/fixtures/single-language/php-composer/` with composer.json + minimal .php file
- [x] 1.3 Create `tests/fixtures/single-language/cpp-cmake/` with CMakeLists.txt + minimal .cpp file
- [x] 1.4 Create `tests/fixtures/single-language/elixir-mix/` with mix.exs + minimal .ex file
- [x] 1.5 Create expected outputs in `tests/fixtures/expected/` for all 4 languages
- [x] 1.6 Add e2e tests for all 4 new fixtures in `tests/e2e.rs`

### 2. Remove Legacy Analyzer System (0.5 hours) ✅
- [x] 2.1 Delete `src/detection/analyzer.rs` (720 lines)
- [x] 2.2 Delete `tests/analyzer_integration.rs` (482 lines)
- [x] 2.3 Delete entire `src/detection/types.rs` (114 lines - only contained GitInfo and RepositoryContext)
- [x] 2.4 Delete entire `tests/error_handling_test.rs` (457 lines - all tests were analyzer-specific)
- [x] 2.5 Remove analyzer exports from `src/detection/mod.rs`
- [x] 2.6 Remove analyzer from public API in `src/lib.rs`
- [x] 2.7 Run `cargo check` to verify no broken references
- [x] 2.8 Verified ~1,773 lines removed

### 3. Infrastructure Simplification (2-3 hours)
- [ ] 3.1 Delete `src/pipeline/context.rs` (130 lines)
- [ ] 3.2 Delete `src/pipeline/config.rs` (64 lines)
- [ ] 3.3 Update `src/detection/service.rs` to construct PipelineOrchestrator directly
- [ ] 3.4 Update `src/main.rs` construction accordingly
- [ ] 3.5 Delete `src/extractors/registry.rs` (30 lines)
- [ ] 3.6 Update extractors to accept `&Path` instead of `&ExtractorRegistry`
- [ ] 3.7 Remove `ProgressHandler` trait from `src/progress/handler.rs`, keep `ProgressEvent` enum
- [ ] 3.8 Remove `NoOpHandler`, simplify `LoggingHandler` to direct struct
- [ ] 3.9 Flatten validation system in `src/validation/rules.rs` to direct functions
- [ ] 3.10 Update `src/validation/validator.rs` to call functions directly
- [ ] 3.11 Remove `intermediate_responses` field from `src/llm/recording.rs`
- [ ] 3.12 Run `cargo check && cargo test` after each substage

### 4. Extractor Consolidation (1-2 hours)
- [ ] 4.1 Create `src/extractors/common.rs` with shared scanning logic
- [ ] 4.2 Extract `scan_directory_with_patterns<F>()` function for common pattern
- [ ] 4.3 Update `src/extractors/port.rs` to use common scanning
- [ ] 4.4 Update `src/extractors/env_vars.rs` to use common scanning
- [ ] 4.5 Update `src/extractors/health.rs` to use common scanning
- [ ] 4.6 Run `cargo test` to validate extractor behavior unchanged

### 5. Registry Optimization (1 hour)
- [ ] 5.1 Replace O(n²) deduplication in `src/languages/registry.rs` with HashSet (O(n))
- [ ] 5.2 Update `all_excluded_dirs()` to use HashSet
- [ ] 5.3 Update `all_workspace_configs()` to use HashSet
- [ ] 5.4 Replace 21-line pattern matching with `glob::Pattern` (3 lines)
- [ ] 5.5 Add `glob = "0.3"` dependency to `Cargo.toml`
- [ ] 5.6 Simplify `detect()` to single-pass algorithm
- [ ] 5.7 Run `cargo test` to validate registry behavior

### 6. Scanner Optimization (1-2 hours)
- [ ] 6.1 Update `src/bootstrap/scanner.rs` to use `WalkDir::min_depth(1)`
- [ ] 6.2 Add early-exit when `files_scanned >= max_files`
- [ ] 6.3 Combine `is_workspace_config` and `is_manifest` checks into single pass
- [ ] 6.4 Consider using `ignore` crate for proper gitignore handling
- [ ] 6.5 Add `ignore = "0.4"` dependency to `Cargo.toml` if using ignore crate
- [ ] 6.6 Run `cargo test` to validate scanner behavior

### 7. Confidence Consolidation (1 hour)
- [ ] 7.1 Create `src/pipeline/confidence.rs` with shared Confidence enum
- [ ] 7.2 Update all 11 pipeline phase files to `use crate::pipeline::confidence::Confidence`
- [ ] 7.3 Remove local Confidence enum definitions from each phase file
- [ ] 7.4 Remove conversion boilerplate from `src/pipeline/phases/15_assemble.rs`
- [ ] 7.5 Update `src/pipeline/mod.rs` exports
- [ ] 7.6 Run `cargo check` to verify all references updated
- [ ] 7.7 Run `cargo test` to ensure behavior unchanged

## Phase B: Architectural (8-12 hours)

### 8. Build System Extraction (8-12 hours)
- [ ] 8.1 Create `src/build_systems/mod.rs` with BuildSystem trait and BuildTemplate struct
- [ ] 8.2 Create `src/build_systems/registry.rs` with BuildSystemRegistry
- [ ] 8.3 Implement build system modules (one commit per build system):
  - [ ] 8.3.1 `src/build_systems/cargo.rs` (Rust)
  - [ ] 8.3.2 `src/build_systems/maven.rs` (Java, Kotlin, Scala)
  - [ ] 8.3.3 `src/build_systems/gradle.rs` (Java, Kotlin)
  - [ ] 8.3.4 `src/build_systems/npm.rs` (JavaScript, TypeScript)
  - [ ] 8.3.5 `src/build_systems/yarn.rs` (JavaScript, TypeScript)
  - [ ] 8.3.6 `src/build_systems/pnpm.rs` (JavaScript, TypeScript)
  - [ ] 8.3.7 `src/build_systems/bun.rs` (JavaScript, TypeScript)
  - [ ] 8.3.8 `src/build_systems/pip.rs` (Python)
  - [ ] 8.3.9 `src/build_systems/poetry.rs` (Python)
  - [ ] 8.3.10 `src/build_systems/pipenv.rs` (Python)
  - [ ] 8.3.11 `src/build_systems/go.rs` (Go)
  - [ ] 8.3.12 `src/build_systems/dotnet.rs` (C#)
  - [ ] 8.3.13 `src/build_systems/composer.rs` (PHP)
- [ ] 8.4 Update Language trait in `src/languages/mod.rs`:
  - [ ] 8.4.1 Add `compatible_build_systems()` method
  - [ ] 8.4.2 Remove `build_template()` method
  - [ ] 8.4.3 Remove `build_systems()` method
  - [ ] 8.4.4 Remove `manifest_files()` method
- [ ] 8.5 Update all 10 language implementations to use new trait
- [ ] 8.6 Update `src/bootstrap/scanner.rs` to use BuildSystemRegistry
- [ ] 8.7 Update pipeline phases to query BuildSystemRegistry directly
- [ ] 8.8 Run `cargo check` after each build system implementation
- [ ] 8.9 Run `cargo test` after all build systems complete
- [ ] 8.10 Run `cargo test --test e2e` to validate all fixtures

## Phase C: Consolidation (4-6 hours)

### 9. Language Consolidation (4-6 hours)
- [ ] 9.1 Create `src/languages/parsers.rs` with parser traits:
  - [ ] 9.1.1 Define `DependencyParser` trait
  - [ ] 9.1.2 Implement `TomlDependencyParser`
  - [ ] 9.1.3 Implement `JsonDependencyParser`
  - [ ] 9.1.4 Implement `RegexDependencyParser`
- [ ] 9.2 Create `src/languages/macros.rs` with `impl_language!` macro
- [ ] 9.3 Refactor language files to use macro (one commit per language):
  - [ ] 9.3.1 `src/languages/rust.rs`
  - [ ] 9.3.2 `src/languages/javascript.rs`
  - [ ] 9.3.3 `src/languages/python.rs`
  - [ ] 9.3.4 `src/languages/java.rs`
  - [ ] 9.3.5 `src/languages/kotlin.rs` (if exists)
  - [ ] 9.3.6 `src/languages/go.rs`
  - [ ] 9.3.7 `src/languages/dotnet.rs`
  - [ ] 9.3.8 `src/languages/ruby.rs`
  - [ ] 9.3.9 `src/languages/php.rs`
  - [ ] 9.3.10 `src/languages/cpp.rs`
  - [ ] 9.3.11 `src/languages/elixir.rs`
- [ ] 9.4 Extract common pattern methods to Language trait with default implementations
- [ ] 9.5 Override pattern methods only where needed
- [ ] 9.6 Run `cargo test` after each language refactor
- [ ] 9.7 Run `cargo test --test e2e` to validate all fixtures

## Final Validation (1 hour)

### 10. Cleanup and Documentation (1 hour)
- [ ] 10.1 Run `cargo clippy -- -W unused` to find dead code
- [ ] 10.2 Run `cargo fix --allow-dirty` to auto-fix warnings
- [ ] 10.3 Update `CLAUDE.md` to reflect new architecture (remove mentions of removed abstractions)
- [ ] 10.4 Update `PRD.md` if architecture changed
- [ ] 10.5 Update `CHANGELOG.md` with refactoring notes
- [ ] 10.6 Run full test suite: `cargo test`
- [ ] 10.7 Run with replay mode: `AIPACK_RECORDING_MODE=replay cargo test`
- [ ] 10.8 Run e2e tests: `cargo test --test e2e`
- [ ] 10.9 Validate detection on polyglot fixture: `cargo run -- detect tests/fixtures/monorepo/polyglot`
- [ ] 10.10 Final validation: `openspec validate refactor-pipeline-codebase --strict`

## Success Metrics
- [ ] All tests pass (`cargo test`)
- [ ] All 14 language fixtures validate (10 existing + 4 new)
- [ ] No clippy warnings
- [ ] ~1,600 lines net reduction achieved
- [ ] Recording system works in replay mode
- [ ] Build systems are first-class entities
- [ ] No Confidence duplication across phases
