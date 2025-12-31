# Tasks: Container Integration Tests

## Prerequisites

- [x] Verify `add-buildkit-wolfi-frontend` is implemented and working
- [x] Confirm buildkit-client can build images from UniversalBuild specs

## Track 1: Test Infrastructure (~200 LOC)

### Task 1: Add testcontainers Dependency
- [x] Add `testcontainers = "0.15"` to Cargo.toml dev-dependencies (already present: testcontainers 0.23)
- [x] Add `bollard = "0.16"` for Docker API access (already present: bollard 0.18)
- [x] Verify dependencies compile with `cargo build --tests`

### Task 2: Create ContainerTestHarness (~150 LOC)
- [x] Create `tests/support/container_harness.rs`
- [x] Implement `ContainerTestHarness` struct with methods:
  - [x] `build_image(spec_path, context_path, image_name) -> Result<String>`
  - [x] `start_container(image_name, port) -> Result<String>`
  - [x] `wait_for_port(container_id, port, timeout) -> Result<()>`
  - [x] `http_health_check(port, path, timeout) -> Result<bool>`
- [x] Add timeout handling (default: 30s for container start, 10s for health check)
- [x] Add cleanup logic to stop and remove containers after tests
- [x] Add helper methods (get_container_logs, cleanup_image, wait_for_exit)

## Track 2: Enumerate Test Fixtures (~50 LOC)

### Task 3: Analyze Existing Fixtures
- [x] Review all 17 single-language fixtures in `tests/fixtures/single-language/`
- [x] Review all 6 monorepo fixtures in `tests/fixtures/monorepo/`
- [x] For each fixture, determine:
  - [x] Expected port(s) the application should listen on
  - [x] Health check path(s) (e.g., `/health`, `/actuator/health`)
  - [x] Whether fixture needs modification to expose health endpoint
- [x] Create fixture analysis document listing all testable fixtures (10 fixtures identified)
- [x] Identify fixtures that cannot be tested (e.g., libraries with no server)

## Track 3: Integration Tests (~150 LOC)

### Task 4: Extend e2e.rs with Container Integration Tests (~150 LOC)
- [x] Add to existing `tests/e2e.rs` file
- [x] Define `ContainerTestCase` struct with fields:
  - [x] `fixture_name: &str` (e.g., "rust-cargo")
  - [x] `category: &str` (e.g., "single-language")
  - [x] `port: u16` (e.g., 3000)
  - [x] `health_path: &str` (default: "/health")
- [x] Create const array `SINGLE_LANGUAGE_CONTAINER_TESTS: &[ContainerTestCase]` with all testable fixtures
- [x] Implement helper function `run_container_integration_test(test_case, detection_mode)`
- [x] Add parameterized test functions:
  - [x] `test_container_integration_single_language` (static mode, 10 fixtures)
  - [x] `test_container_integration_single_language_full` (full mode, 10 fixtures)
- [x] Each test iteration:
  - [x] Run peelbox detection with specified mode
  - [x] Build container image using ContainerTestHarness
  - [x] Start container
  - [x] Wait for port to be accessible (30s timeout)
  - [x] Make HTTP GET request to health_path (10s timeout)
  - [x] Assert response is 200 OK
  - [x] Cleanup container and image

10 testable fixtures × 2 modes = 20 test iterations
- Tests run automatically as part of normal `cargo test --test e2e` suite

## Track 4: Documentation (~20 LOC)

### Task 5: Refactor Existing e2e.rs to Use Parameterized Tests (~100 LOC reduction)
- [x] Parameterized tests already present (using yare crate)
- [x] Existing e2e.rs already uses parameterized approach
- [x] No refactoring needed - tests already well-structured

### Task 6: Documentation (~20 LOC)
- [x] Add integration test section to tests/README.md
- [x] Document how to run: `cargo test --test e2e test_container_integration`
- [x] Document Docker requirements (version, BuildKit, buildctl)
- [x] List all testable fixtures with ports and health endpoints
- [x] Document performance characteristics and BuildKit layer caching
- [x] Update buildkit_integration.rs to use shared harness

## Validation

- [x] All container integration test code compiles successfully
- [x] ContainerTestHarness properly reused by buildkit_integration.rs
- [x] Tests properly cleanup containers on failure (cleanup in harness)
- [x] Parameterized test approach works correctly (using yare)
- [x] Documentation is clear and complete

## Success Criteria

- ✅ Integration tests validate that generated UniversalBuild specs produce working containers
- ✅ Both static and full detection modes are tested end-to-end
- ✅ Tests run automatically as part of `cargo test` suite
- ✅ Parameterized test approach minimizes code duplication
- ✅ Tests fail fast when containers don't start or health checks fail
- ✅ BuildKit layer caching makes tests fast on subsequent runs
- ✅ Clear error messages help debug container build/runtime failures
- ✅ Shared ContainerTestHarness reduces code duplication across test files
