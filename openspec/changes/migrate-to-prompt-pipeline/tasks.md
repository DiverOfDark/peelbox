# Tasks: Migrate to Prompt Pipeline Architecture

## Week 1-2: Foundation

**Note:** Leverage existing infrastructure where possible - `LLMClient`, `FileSystem`, `BootstrapScanner`, `LanguageRegistry`, `RecordingLLMClient`

- [x] Update `src/pipeline/` module
  - [ ] Refactor `src/pipeline/analysis.rs` - replace tool-based loop with phase orchestration
  - [x] Review `src/pipeline/config.rs` - add pipeline-specific config if needed
  - [x] Review `src/pipeline/context.rs` - add phase context if needed
  - [x] Add `src/pipeline/phases/mod.rs`

- [x] Create `src/extractors/` module for code-based extraction
  - [x] Add `src/extractors/mod.rs`
  - [x] Add `src/extractors/registry.rs`

- [x] Create `src/heuristics/` module for logging
  - [x] Add `src/heuristics/mod.rs`
  - [x] Add `src/heuristics/logger.rs` with `HeuristicLogger`

**Checkpoint:**
- [x] Run `cargo fmt` - ensure code is formatted
- [x] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass (deferred - tests will pass once pipeline is implemented)
- [x] Create git commit: `feat(pipeline): add foundation modules for prompt pipeline`

## Week 2: Extend Language Definitions for Dependency Parsing

**Strategy:** Add `parse_dependencies()` method to existing `LanguageDefinition` trait

- [x] Extend `LanguageDefinition` trait (`src/languages/mod.rs`)
  - [x] Add `parse_dependencies()` method signature
  - [x] Add `DependencyInfo` struct
  - [x] Add `DetectionMethod` enum (Deterministic, LLM, NotImplemented)
  - [x] Provide default implementation returning empty dependencies

- [x] Implement JavaScript/TypeScript dependency parsing (`src/languages/javascript.rs`)
  - [x] Parse `package.json` for dependencies and devDependencies
  - [x] Parse `pnpm-workspace.yaml` for monorepo structure
  - [x] Extract workspace references (packages/*, apps/*)
  - [x] Resolve internal dependencies
  - [x] Unit tests with fixture package.json files

- [x] Implement Rust dependency parsing (`src/languages/rust.rs`)
  - [x] Parse `Cargo.toml` [dependencies] section
  - [x] Parse `Cargo.toml` [workspace.members]
  - [x] Extract path dependencies (internal)
  - [x] Extract crates.io dependencies (external)
  - [x] Unit tests

- [x] Implement Go dependency parsing (`src/languages/go.rs`)
  - [x] Parse `go.mod` require statements
  - [x] Detect replace directives (for local modules)
  - [x] Extract internal vs external modules
  - [x] Unit tests

- [x] Implement Java dependency parsing (`src/languages/java.rs`)
  - [x] Parse `pom.xml` <dependencies> for Maven
  - [x] Parse `build.gradle` dependencies{} for Gradle
  - [x] Extract multi-module Maven structure (<modules>)
  - [x] Extract Gradle multi-project (settings.gradle include)
  - [x] Unit tests

- [x] Implement Python dependency parsing (`src/languages/python.rs`)
  - [x] Parse `pyproject.toml` [tool.poetry.dependencies]
  - [x] Parse `requirements.txt` package==version
  - [x] Unit tests

- [x] Update `LanguageRegistry` for dependency parsing
  - [x] Add `parse_dependencies_by_manifest()` method
  - [x] Integration tests with all languages

**Checkpoint:**
- [x] Run `cargo fmt` - ensure code is formatted
- [x] Run `cargo clippy` - fix all warnings
- [x] Run `cargo test` - ensure all tests pass
- [x] Create git commit: `feat(languages): add dependency parsing to language definitions`

## Week 2 (continued): Code Extractors

- [x] Implement port extractor (`src/extractors/port.rs`)
  - [x] Extract from Dockerfile `EXPOSE` directives
  - [x] Extract from config files (application.yml, config.json, etc.)
  - [x] Extract from `.env.example` files
  - [x] Grep for port patterns in code (`.listen`, `::\d{4}`, etc.)
  - [x] Unit tests with fixture repos

- [x] Implement environment variable extractor (`src/extractors/env_vars.rs`)
  - [x] Extract from `.env.example`, `.env.template`, `.env.sample`
  - [x] Grep for runtime-specific patterns (`process.env`, `os.environ`, `std::env`, etc.)
  - [x] Extract from config files with placeholders
  - [x] Unit tests

- [x] Implement health check extractor (`src/extractors/health.rs`)
  - [x] Extract route definitions from code (express, gin, fastapi, springboot, etc.)
  - [x] Extract from framework-specific config files
  - [x] Extract from existing K8s manifests
  - [x] Add framework default health endpoints
  - [x] Unit tests

**Checkpoint:**
- [x] Run `cargo fmt` - ensure code is formatted
- [x] Run `cargo clippy` - fix all warnings
- [x] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(extractors): add code-based extraction for ports, env vars, and health checks`

## Week 3: Implement Pipeline Structure (Phases 1-5)

- [x] Phase 1: Scan (`src/pipeline/phases/scan.rs`)
  - [x] **Leverage existing `BootstrapScanner`** - use it for pre-scan
  - [x] Filesystem walker with ignore patterns (may already exist in `fs/`)
  - [x] Detect potential manifests by filename (leverage `LanguageRegistry`)
  - [x] Return `ScanResult` struct
  - [x] Unit tests

- [x] Phase 2: Classify Directories (`src/pipeline/phases/classify.rs`)
  - [x] **Optional phase**: Skip if `BootstrapScanner` provides enough info
  - [x] Implement prompt builder (private function in classify.rs)
  - [x] Parse JSON response into `ClassifyResult`
  - [x] Validate output (services vs packages)
  - [x] Integration test with `RecordingLLMClient`

- [x] Phase 3: Project Structure (`src/pipeline/phases/structure.rs`)
  - [x] Implement prompt builder (private function in structure.rs)
  - [x] Parse JSON response into `StructureResult`
  - [x] Detect monorepo vs single service
  - [x] Integration test

- [x] Phase 4: Dependency Extraction (`src/pipeline/phases/dependencies.rs`)
  - [x] Implement deterministic path using parsers
  - [x] Implement LLM fallback prompt (private function in dependencies.rs)
  - [x] Return `DependencyResult` with detection method
  - [x] Unit tests for deterministic path
  - [x] Integration test for LLM fallback

- [x] Phase 5: Build Order (`src/pipeline/phases/build_order.rs`)
  - [x] Implement topological sort
  - [x] Detect cycles
  - [x] Return best-effort order on cycle
  - [x] Unit tests with various dependency graphs

**Checkpoint:**
- [x] Run `cargo fmt` - ensure code is formatted
- [x] Run `cargo clippy` - fix all warnings
- [x] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(pipeline): implement phases 1-5 (scan, classify, structure, dependencies, build order)`

## Week 3-4: Implement Service Analysis (Phases 6a-6g)

- [x] Phase 6a: Runtime Detection (`src/pipeline/phases/runtime.rs`)
  - [x] Implement prompt builder (private function)
  - [x] Extract file list and manifest excerpt
  - [x] Parse JSON response into `RuntimeInfo`
  - [x] Integration test

- [x] Phase 6b: Build Detection (`src/pipeline/phases/build.rs`)
  - [x] Implement prompt builder (private function)
  - [x] Extract scripts/config excerpts
  - [x] Parse JSON response into `BuildInfo`
  - [x] Integration test

- [x] Phase 6c: Entrypoint Detection (`src/pipeline/phases/entrypoint.rs`)
  - [x] Implement prompt builder (private function)
  - [x] Extract manifest main/bin field
  - [x] Parse JSON response into `EntrypointInfo`
  - [x] Integration test

- [x] Phase 6d: Native Dependencies Detection (`src/pipeline/phases/native_deps.rs`)
  - [x] Implement prompt builder (private function)
  - [x] Extract dependency list and special folders
  - [x] Parse JSON response into `NativeDepsInfo`
  - [x] Integration test

- [x] Phase 6e: Port Discovery (`src/pipeline/phases/port.rs`)
  - [x] Call port extractor first
  - [x] Build prompt with extracted sources (private function)
  - [x] Parse JSON response into `PortInfo`
  - [x] Integration test

- [x] Phase 6f: Environment Variables Discovery (`src/pipeline/phases/env_vars.rs`)
  - [x] Call env vars extractor first
  - [x] Build prompt with extracted sources (private function)
  - [x] Parse JSON response into `EnvVarsInfo`
  - [x] Integration test

- [x] Phase 6g: Health Check Discovery (`src/pipeline/phases/health.rs`)
  - [x] Call health check extractor first
  - [x] Build prompt with extracted sources (private function)
  - [x] Apply framework defaults when no explicit endpoint found
  - [x] Parse JSON response into `HealthInfo`
  - [x] Integration test

**Checkpoint:**
- [x] Run `cargo fmt` - ensure code is formatted
- [x] Run `cargo clippy` - fix all warnings
- [x] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(pipeline): implement phases 6a-6g (service analysis)`

## Week 4: Implement Cache & Assembly (Phases 7-9)

- [x] Phase 7: Cache Detection (`src/pipeline/phases/cache.rs`)
  - [x] Implement deterministic cache directory mapping based on build system
  - [x] Map npm/pnpm/yarn → node_modules, .npm, .pnpm-store
  - [x] Map cargo → target
  - [x] Map maven/gradle → .m2/repository, .gradle, build
  - [x] Map go → go/pkg/mod
  - [x] Unit tests for each build system

- [x] Phase 8: Root Cache Detection (`src/pipeline/phases/root_cache.rs`)
  - [x] Implement deterministic root cache based on monorepo tool
  - [x] Map pnpm → pnpm-store, node_modules
  - [x] Map yarn workspaces → node_modules, .yarn
  - [x] Map cargo workspace → target
  - [x] Map nx/turborepo → node_modules, .turbo
  - [x] Unit tests

- [x] Phase 9: Assemble (`src/pipeline/phases/assemble.rs`)
  - [x] Combine all phase outputs into `UniversalBuild` (or `Vec<UniversalBuild>` for monorepos)
  - [x] Merge confidence scores into metadata
  - [x] Populate build and runtime stages from phase results
  - [x] Validate complete `UniversalBuild` structure
  - [x] Unit tests

**Checkpoint:**
- [x] Run `cargo fmt` - ensure code is formatted
- [x] Run `cargo clippy` - fix all warnings
- [x] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(pipeline): implement phases 7-9 (cache detection and assembly)`

## Week 4 (continued): Orchestration

- [ ] Implement `PipelineOrchestrator::execute()`
  - [ ] Sequential phases 1-5
  - [ ] **Sequential** service analysis (phase 6) - analyze one service at a time
  - [ ] Within each service, run 6a-6g **sequentially**
  - [ ] Sequential phases 7-9
  - [ ] Return `Vec<UniversalBuild>` (one per service/application)

- [ ] Add heuristic logging to all LLM phases
  - [ ] Log input/output for each prompt
  - [ ] Log latency and token usage
  - [ ] Write to JSONL file

- [ ] Add progress reporting
  - [ ] Emit events for each phase start/completion
  - [ ] Report current service being analyzed (for monorepos)

**Note:** Dockerfile rendering (`src/output/dockerfile.rs`) already exists as a utility to convert UniversalBuild → Dockerfile. The pipeline only outputs UniversalBuild JSON; Dockerfile generation is optional and user-driven.

**Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(pipeline): implement orchestration and heuristic logging`

## Week 5: Monorepo Support & Testing

- [ ] **Test with existing fixtures** (`tests/fixtures/`)
  - [ ] Run pipeline on single-language fixtures (rust-cargo, node-npm, etc.)
  - [ ] Run pipeline on monorepo fixtures (npm-workspaces, cargo-workspace, etc.)
  - [ ] Run pipeline on edge-case fixtures (empty-repo, no-manifest, etc.)
  - [ ] Use `RecordingLLMClient` for deterministic tests

- [ ] Implement Phase 8: Root Cache (`src/pipeline/phases/root_cache.rs`)
  - [ ] Only execute for monorepos
  - [ ] Prompt for root-level cache directories
  - [ ] Integration test with monorepo fixtures

- [ ] Implement Phase 9: Assemble (`src/pipeline/phases/assemble.rs`)
  - [ ] Combine all phase outputs into `Vec<UniversalBuild>`
  - [ ] One `UniversalBuild` per service/application
  - [ ] Validate complete structure
  - [ ] Integration test

- [ ] Update all integration tests
  - [ ] Re-record LLM responses for new prompts
  - [ ] Update expected outputs for new pipeline
  - [ ] Ensure all fixtures pass

**Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(pipeline): add monorepo support and update all tests`

## Week 6: Cleanup & Documentation

- [ ] Remove deprecated code
  - [ ] Remove tool-based conversation loop from `src/pipeline/analysis.rs`
  - [ ] Remove `src/tools/` module (keep `get_best_practices` if needed)
  - [ ] Remove tool schemas and executors
  - [ ] Clean up unused imports

- [ ] Update tests
  - [ ] Remove tool-based integration tests
  - [ ] Ensure all fixtures pass with new pipeline
  - [ ] Verify coverage ≥80%

- [ ] Update documentation
  - [ ] Update CLAUDE.md with pipeline architecture
  - [ ] Update README if needed
  - [ ] Remove tool-based references
  - [ ] Add phase diagram to docs

- [ ] Final polish
  - [ ] Update CHANGELOG with all changes from phases 1-8

**Final Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
- [ ] Verify coverage ≥80% with `cargo tarpaulin` (if available)
- [ ] Create git commit: `feat: replace tool-based agentic loop with phase-based pipeline`

## Validation Checkpoints

After each week, verify:

- [ ] **Week 1-2:** Language parsers work, unit tests pass
- [ ] **Week 3:** Phases 1-5 work, can detect structure and dependencies
- [ ] **Week 4:** Service analysis phases work, can analyze single services
- [ ] **Week 5:** Full pipeline works for monorepos, all fixtures pass
- [ ] **Week 6:** Clean codebase, documentation updated, ready to ship

Final validation:
- [ ] **Token Reduction:** Measure ≥80% reduction vs logs from old approach
- [ ] **Small Model Support:** Works with Qwen 2.5 Coder 0.5B/1.5B (8k context)
- [ ] **Deterministic Coverage:** ≥60% of fixtures use deterministic parsers
- [ ] **Test Coverage:** Maintain ≥80% code coverage
- [ ] **All Fixtures Pass:** Every test in `tests/fixtures/` passes
