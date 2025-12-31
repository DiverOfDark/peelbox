# Dual-Mode Testing Proposal

## Change ID
`dual-mode-testing`

## Summary
Enable comprehensive e2e testing of both LLM and static analysis detection modes by:
1. Adding environment variable `PEELBOX_DETECTION_MODE` to control detection mode (llm/static/full)
2. Creating dual-mode e2e test variants for each fixture (spawn CLI with different modes)
3. Ensuring every fixture can be validated via both LLM and static analysis paths through CLI

## Why
The current test suite has critical coverage gaps:
1. **No static analysis validation**: E2e tests only run with LLM (embedded model), we can't verify static analysis works for all supported languages
2. **Slow CI**: All tests require LLM backend, making CI slow and expensive
3. **Cannot validate both paths**: No way to test that CLI properly supports both detection modes
4. **Missing fast path for CI**: Cannot run deterministic-only tests without LLM backend

This change adds dual-mode e2e tests (spawning CLI binary with mode control) to validate both code paths.

## Problem Statement
Currently, e2e tests have gaps:

1. **No Static Analysis Coverage**: E2e tests only run with LLM (embedded model via `PEELBOX_PROVIDER=embedded`). No tests validate static analysis paths work correctly.
2. **No Mode Control**: Cannot control detection mode via CLI to test different code paths
3. **Slow CI**: All e2e tests require LLM backend, making test suite slow (minutes vs seconds)
4. **Cannot Test Both Modes**: No way to verify that CLI properly executes both LLM and static analysis paths

The pipeline already supports both modes internally (e.g., `deterministic_classify` in classify phase), but the CLI doesn't expose mode control and tests don't validate both paths.

## Goals
1. **Dual-Mode E2e Tests**: Add e2e test variants for every fixture in both LLM mode and static-only mode
2. **CLI Mode Control**: Add `PEELBOX_DETECTION_MODE` environment variable to control detection mode
3. **Complete Coverage**: Validate that static analysis fallbacks work for all supported languages/build systems via CLI
4. **Fast CI**: Static-only e2e tests run without LLM backend (< 10 seconds for all fixtures)
5. **Clear Test Organization**:
   - E2e tests spawn CLI binary with `PEELBOX_DETECTION_MODE=static` or `PEELBOX_DETECTION_MODE=llm`
   - Each fixture has two test variants: `test_rust_cargo_llm()` and `test_rust_cargo_static()`
   - All tests remain e2e tests (no unit tests with MockFileSystem)

## Non-Goals
- Changing the core pipeline architecture (it already supports both modes)
- Adding new language or build system support
- Modifying fixture content (fixtures are already minimal and representative)
- Performance optimization (focus is on correctness and coverage)

## Scope
### In Scope
- Add `PEELBOX_DETECTION_MODE` environment variable (values: `llm`, `static`, `full`)
- Add `DetectionMode` parameter to PipelineOrchestrator based on env var
- Create dual e2e test variants in `tests/e2e.rs` for each fixture (spawn CLI with different modes)
- Ensure all pipeline phases properly handle static-only execution
- Add static-mode expected JSON files where needed
- All tests remain e2e tests spawning the CLI binary

### Out of Scope
- Creating unit tests with MockFileSystem (all tests are e2e)
- Refactoring pipeline phases (they already have deterministic paths)
- Adding new test fixtures
- Changing LLM recording system
- Modifying cli_integration.rs or mock_detection_test.rs
- CI/CD pipeline changes (tests can be run manually with different modes)

## Dependencies
- Depends on current pipeline architecture (phase-based, deterministic-first)
- Requires MockFileSystem trait (already exists in `src/fs/`)
- Uses existing expected JSON outputs in `tests/fixtures/expected/`

## Success Criteria
1. 50+ e2e tests in `tests/e2e.rs` (25 fixtures × 2 modes)
2. All e2e tests spawn CLI binary and pass in both LLM and static modes
3. CLI respects `PEELBOX_DETECTION_MODE` environment variable
4. Static-mode e2e tests run without LLM backend (< 10 seconds)
5. Static mode e2e tests are deterministic and fast
6. All tests remain e2e tests (no unit tests, no MockFileSystem)

## Related Changes
- `unify-registry-chain` - Stack registry refactoring provides foundation for deterministic detection
- Future: May enable "fast path" detection for CI/CD pipelines (static-only)

## Risks & Mitigation
### Risk: Some fixtures may not be detectable via static analysis alone
**Mitigation**: Use LLM fallback for those cases. Static mode tests can accept lower confidence or partial detection.

### Risk: Maintaining dual test variants increases test maintenance burden
**Mitigation**: Use shared validation helpers. E2e tests remain unchanged, so only unit tests are added (not replaced).

### Risk: Test duplication between e2e and unit tests
**Mitigation**: Different purposes - e2e tests validate CLI UX and full binary, unit tests validate detection logic and dual modes. Both are valuable.

## Implementation Notes
- Add `PEELBOX_DETECTION_MODE` environment variable to control mode
- CLI reads env var and passes `DetectionMode` to PipelineOrchestrator
- E2e tests spawn binary with `PEELBOX_DETECTION_MODE=static` or `PEELBOX_DETECTION_MODE=llm`
- Each phase already has deterministic fallbacks; static mode e2e tests exercise them
- All tests remain in `tests/e2e.rs` (no separate unit test file)
- Expected JSON may need mode-specific variants (e.g., `rust-cargo-static.json`)
