# Tasks - Dual-Mode Testing

## Phase 1: CLI Mode Control
- [x] Add `DetectionMode` enum: `Full` (LLM + static), `StaticOnly`, `LLMOnly`
- [x] Add `AIPACK_DETECTION_MODE` environment variable parsing in `src/config.rs`
- [x] Pass `DetectionMode` from CLI to `PipelineOrchestrator`
- [x] Add unit tests for env var parsing (test that "static", "llm", "full" map correctly)

## Phase 2: Pipeline Mode Support
- [x] Add `mode: DetectionMode` parameter to `PipelineOrchestrator::new()` or `::execute()`
- [x] Update each phase's `execute()` to check mode before LLM calls:
  - [x] Phase 2 (classify): Skip LLM if StaticOnly, use deterministic path
  - [x] Phase 3 (structure): Skip LLM if StaticOnly
  - [x] Phase 4 (dependencies): Skip LLM if StaticOnly
  - [x] Phase 6 (runtime): Skip LLM if StaticOnly
  - [x] Phase 7 (build): Skip LLM if StaticOnly
  - [x] Phase 8 (entrypoint): Skip LLM if StaticOnly
  - [x] Phase 9 (native_deps): Skip LLM if StaticOnly
  - [x] Phase 10 (port): Already has extractors, ensure static path works
  - [x] Phase 11 (env_vars): Already has extractors, ensure static path works
  - [x] Phase 12 (health): Already has extractors, ensure static path works
- [x] Verify pipeline completes in static mode without calling LLM

## Phase 3: E2e Test Helpers
- [x] Update `run_detection()` helper in `tests/e2e.rs` to accept mode parameter
- [x] Add `run_detection_llm(fixture)` helper that sets `AIPACK_DETECTION_MODE=llm`
- [x] Add `run_detection_static(fixture)` helper that sets `AIPACK_DETECTION_MODE=static`
- [x] Update `assert_detection()` to accept mode parameter for mode-specific validation

## Phase 4: Add Dual-Mode E2e Tests
- [ ] Single-language fixtures (17 × 2 = 34 tests):
  - [ ] `test_rust_cargo_llm()` and `test_rust_cargo_static()`
  - [ ] `test_node_npm_llm()` and `test_node_npm_static()`
  - [ ] `test_python_pip_llm()` and `test_python_pip_static()`
  - [ ] `test_java_maven_llm()` and `test_java_maven_static()`
  - [ ] `test_node_yarn_llm()` and `test_node_yarn_static()`
  - [ ] `test_node_pnpm_llm()` and `test_node_pnpm_static()`
  - [ ] `test_python_poetry_llm()` and `test_python_poetry_static()`
  - [ ] `test_java_gradle_llm()` and `test_java_gradle_static()`
  - [ ] `test_kotlin_gradle_llm()` and `test_kotlin_gradle_static()`
  - [ ] `test_dotnet_csproj_llm()` and `test_dotnet_csproj_static()`
  - [ ] `test_go_mod_llm()` and `test_go_mod_static()`
  - [ ] `test_ruby_bundler_llm()` and `test_ruby_bundler_static()`
  - [ ] `test_php_composer_llm()` and `test_php_composer_static()`
  - [ ] `test_cpp_cmake_llm()` and `test_cpp_cmake_static()`
  - [ ] `test_elixir_mix_llm()` and `test_elixir_mix_static()`
  - [ ] `test_rust_workspace_llm()` and `test_rust_workspace_static()`
  - [ ] `test_turborepo_llm()` and `test_turborepo_static()`
- [ ] Monorepo fixtures (7 × 2 = 14 tests):
  - [ ] `test_npm_workspaces_llm()` and `test_npm_workspaces_static()`
  - [ ] `test_cargo_workspace_llm()` and `test_cargo_workspace_static()`
  - [ ] `test_gradle_multiproject_llm()` and `test_gradle_multiproject_static()`
  - [ ] `test_maven_multimodule_llm()` and `test_maven_multimodule_static()`
  - [ ] `test_polyglot_llm()` and `test_polyglot_static()`
- [ ] Edge case tests (2 × 2 = 4 tests):
  - [ ] `test_empty_repo_static()` and `test_empty_repo_llm()`
  - [ ] `test_no_manifest_static()` and `test_no_manifest_llm()`

## Phase 5: Expected Outputs
- [ ] Review expected JSON files for static-mode compatibility
- [ ] Create mode-specific expected outputs if needed (e.g., `rust-cargo-static.json`)
- [ ] Add validation that static mode produces valid UniversalBuild output
- [ ] Document differences between LLM and static mode outputs (confidence, optional fields)

## Phase 6: Documentation & Cleanup
- [ ] Update CLAUDE.md with detection mode control:
  - E2e tests spawn CLI with `AIPACK_DETECTION_MODE` env var
  - Static mode runs without LLM backend (fast, deterministic)
  - LLM mode validates LLM code paths
- [ ] Document when to use each mode in testing
- [ ] Add examples of running e2e tests in different modes
- [ ] Verify all tests pass in all modes
- [ ] Update CHANGELOG.md

## Validation Checkpoints
After each phase:
1. Run `cargo test` to ensure no regressions
2. Run `cargo clippy` to check for warnings
3. Verify static mode e2e tests run without LLM backend
4. Check test execution time (static mode should be < 10 seconds for all tests)
