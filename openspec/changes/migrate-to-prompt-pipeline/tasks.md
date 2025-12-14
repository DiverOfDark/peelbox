# Tasks: Migrate to Prompt Pipeline Architecture

## Week 1-2: Foundation

**Note:** Leverage existing infrastructure where possible - `LLMClient`, `FileSystem`, `BootstrapScanner`, `LanguageRegistry`, `RecordingLLMClient`

- [ ] Update `src/pipeline/` module
  - [ ] Refactor `src/pipeline/analysis.rs` - replace tool-based loop with phase orchestration
  - [ ] Review `src/pipeline/config.rs` - add pipeline-specific config if needed
  - [ ] Review `src/pipeline/context.rs` - add phase context if needed
  - [ ] Add `src/pipeline/phases/mod.rs`

- [ ] Create `src/extractors/` module for code-based extraction
  - [ ] Add `src/extractors/mod.rs`
  - [ ] Add `src/extractors/registry.rs`

- [ ] Create `src/heuristics/` module for logging
  - [ ] Add `src/heuristics/mod.rs`
  - [ ] Add `src/heuristics/logger.rs` with `HeuristicLogger`

**Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(pipeline): add foundation modules for prompt pipeline`

## Week 2: Extend Language Definitions for Dependency Parsing

**Strategy:** Add `parse_dependencies()` method to existing `LanguageDefinition` trait

- [ ] Extend `LanguageDefinition` trait (`src/languages/mod.rs`)
  - [ ] Add `parse_dependencies()` method signature
  - [ ] Add `DependencyInfo` struct
  - [ ] Add `DetectionMethod` enum (Deterministic, LLM, NotImplemented)
  - [ ] Provide default implementation returning empty dependencies

- [ ] Implement JavaScript/TypeScript dependency parsing (`src/languages/javascript.rs`)
  - [ ] Parse `package.json` for dependencies and devDependencies
  - [ ] Parse `pnpm-workspace.yaml` for monorepo structure
  - [ ] Extract workspace references (packages/*, apps/*)
  - [ ] Resolve internal dependencies
  - [ ] Unit tests with fixture package.json files

- [ ] Implement Rust dependency parsing (`src/languages/rust.rs`)
  - [ ] Parse `Cargo.toml` [dependencies] section
  - [ ] Parse `Cargo.toml` [workspace.members]
  - [ ] Extract path dependencies (internal)
  - [ ] Extract crates.io dependencies (external)
  - [ ] Unit tests

- [ ] Implement Go dependency parsing (`src/languages/go.rs`)
  - [ ] Parse `go.mod` require statements
  - [ ] Detect replace directives (for local modules)
  - [ ] Extract internal vs external modules
  - [ ] Unit tests

- [ ] Implement Java dependency parsing (`src/languages/java.rs`)
  - [ ] Parse `pom.xml` <dependencies> for Maven
  - [ ] Parse `build.gradle` dependencies{} for Gradle
  - [ ] Extract multi-module Maven structure (<modules>)
  - [ ] Extract Gradle multi-project (settings.gradle include)
  - [ ] Unit tests

- [ ] Implement Python dependency parsing (`src/languages/python.rs`)
  - [ ] Parse `pyproject.toml` [tool.poetry.dependencies]
  - [ ] Parse `requirements.txt` package==version
  - [ ] Unit tests

- [ ] Update `LanguageRegistry` for dependency parsing
  - [ ] Add `parse_dependencies_by_manifest()` method
  - [ ] Integration tests with all languages

**Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(languages): add dependency parsing to language definitions`

## Week 2 (continued): Code Extractors

- [ ] Implement port extractor (`src/extractors/port.rs`)
  - [ ] Extract from Dockerfile `EXPOSE` directives
  - [ ] Extract from config files (application.yml, config.json, etc.)
  - [ ] Extract from `.env.example` files
  - [ ] Grep for port patterns in code (`.listen`, `::\d{4}`, etc.)
  - [ ] Unit tests with fixture repos

- [ ] Implement environment variable extractor (`src/extractors/env_vars.rs`)
  - [ ] Extract from `.env.example`, `.env.template`, `.env.sample`
  - [ ] Grep for runtime-specific patterns (`process.env`, `os.environ`, `std::env`, etc.)
  - [ ] Extract from config files with placeholders
  - [ ] Unit tests

- [ ] Implement health check extractor (`src/extractors/health.rs`)
  - [ ] Extract route definitions from code (express, gin, fastapi, springboot, etc.)
  - [ ] Extract from framework-specific config files
  - [ ] Extract from existing K8s manifests
  - [ ] Add framework default health endpoints
  - [ ] Unit tests

**Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(extractors): add code-based extraction for ports, env vars, and health checks`

## Week 3: Implement Pipeline Structure (Phases 1-5)

- [ ] Phase 1: Scan (`src/pipeline/phases/scan.rs`)
  - [ ] **Leverage existing `BootstrapScanner`** - use it for pre-scan
  - [ ] Filesystem walker with ignore patterns (may already exist in `fs/`)
  - [ ] Detect potential manifests by filename (leverage `LanguageRegistry`)
  - [ ] Return `ScanResult` struct
  - [ ] Unit tests

- [ ] Phase 2: Classify Directories (`src/pipeline/phases/classify.rs`)
  - [ ] **Optional phase**: Skip if `BootstrapScanner` provides enough info
  - [ ] Implement prompt builder (private function in classify.rs)
  - [ ] Parse JSON response into `ClassifyResult`
  - [ ] Validate output (services vs packages)
  - [ ] Integration test with `RecordingLLMClient`

- [ ] Phase 3: Project Structure (`src/pipeline/phases/structure.rs`)
  - [ ] Implement prompt builder (private function in structure.rs)
  - [ ] Parse JSON response into `StructureResult`
  - [ ] Detect monorepo vs single service
  - [ ] Integration test

- [ ] Phase 4: Dependency Extraction (`src/pipeline/phases/dependencies.rs`)
  - [ ] Implement deterministic path using parsers
  - [ ] Implement LLM fallback prompt (private function in dependencies.rs)
  - [ ] Return `DependencyResult` with detection method
  - [ ] Unit tests for deterministic path
  - [ ] Integration test for LLM fallback

- [ ] Phase 5: Build Order (`src/pipeline/phases/build_order.rs`)
  - [ ] Implement topological sort
  - [ ] Detect cycles
  - [ ] Return best-effort order on cycle
  - [ ] Unit tests with various dependency graphs

**Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(pipeline): implement phases 1-5 (scan, classify, structure, dependencies, build order)`

## Week 3-4: Implement Service Analysis (Phases 6a-6g)

- [ ] Phase 6a: Runtime Detection (`src/pipeline/phases/runtime.rs`)
  - [ ] Implement prompt builder (private function)
  - [ ] Extract file list and manifest excerpt
  - [ ] Parse JSON response into `RuntimeInfo`
  - [ ] Integration test

- [ ] Phase 6b: Build Detection (`src/pipeline/phases/build.rs`)
  - [ ] Implement prompt builder (private function)
  - [ ] Extract scripts/config excerpts
  - [ ] Parse JSON response into `BuildInfo`
  - [ ] Integration test

- [ ] Phase 6c: Entrypoint Detection (`src/pipeline/phases/entrypoint.rs`)
  - [ ] Implement prompt builder (private function)
  - [ ] Extract manifest main/bin field
  - [ ] Parse JSON response into `EntrypointInfo`
  - [ ] Integration test

- [ ] Phase 6d: Native Dependencies Detection (`src/pipeline/phases/native_deps.rs`)
  - [ ] Implement prompt builder (private function)
  - [ ] Extract dependency list and special folders
  - [ ] Parse JSON response into `NativeDepsInfo`
  - [ ] Integration test

- [ ] Phase 6e: Port Discovery (`src/pipeline/phases/port.rs`)
  - [ ] Call port extractor first
  - [ ] Build prompt with extracted sources (private function)
  - [ ] Parse JSON response into `PortInfo`
  - [ ] Integration test

- [ ] Phase 6f: Environment Variables Discovery (`src/pipeline/phases/env_vars.rs`)
  - [ ] Call env vars extractor first
  - [ ] Build prompt with extracted sources (private function)
  - [ ] Parse JSON response into `EnvVarsInfo`
  - [ ] Integration test

- [ ] Phase 6g: Health Check Discovery (`src/pipeline/phases/health.rs`)
  - [ ] Call health check extractor first
  - [ ] Build prompt with extracted sources (private function)
  - [ ] Apply framework defaults when no explicit endpoint found
  - [ ] Parse JSON response into `HealthInfo`
  - [ ] Integration test

**Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
- [ ] Create git commit: `feat(pipeline): implement phases 6a-6g (service analysis)`

## Week 4: Implement Cache & Assembly (Phases 7-9)

- [ ] Phase 7: Cache Detection (`src/pipeline/phases/cache.rs`)
  - [ ] Implement deterministic cache directory mapping based on build system
  - [ ] Map npm/pnpm/yarn → node_modules, .npm, .pnpm-store
  - [ ] Map cargo → target
  - [ ] Map maven/gradle → .m2/repository, .gradle, build
  - [ ] Map go → go/pkg/mod
  - [ ] Unit tests for each build system

- [ ] Phase 8: Root Cache Detection (`src/pipeline/phases/root_cache.rs`)
  - [ ] Implement deterministic root cache based on monorepo tool
  - [ ] Map pnpm → pnpm-store, node_modules
  - [ ] Map yarn workspaces → node_modules, .yarn
  - [ ] Map cargo workspace → target
  - [ ] Map nx/turborepo → node_modules, .turbo
  - [ ] Unit tests

- [ ] Phase 9: Assemble (`src/pipeline/phases/assemble.rs`)
  - [ ] Combine all phase outputs into `UniversalBuild` (or `Vec<UniversalBuild>` for monorepos)
  - [ ] Merge confidence scores into metadata
  - [ ] Populate build and runtime stages from phase results
  - [ ] Validate complete `UniversalBuild` structure
  - [ ] Unit tests

**Checkpoint:**
- [ ] Run `cargo fmt` - ensure code is formatted
- [ ] Run `cargo clippy` - fix all warnings
- [ ] Run `cargo test` - ensure all tests pass
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
