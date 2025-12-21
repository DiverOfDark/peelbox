# Tasks: Container Integration Tests

## Prerequisites

- [ ] Verify `add-buildkit-wolfi-frontend` is implemented and working
- [ ] Confirm buildkit-client can build images from UniversalBuild specs

## Track 1: Test Infrastructure (~200 LOC)

### Task 1: Add testcontainers Dependency
- [ ] Add `testcontainers = "0.15"` to Cargo.toml dev-dependencies
- [ ] Add `bollard = "0.16"` for Docker API access (used by testcontainers)
- [ ] Verify dependencies compile with `cargo build --tests`

### Task 2: Create ContainerTestHarness (~150 LOC)
- [ ] Create `tests/support/container_harness.rs`
- [ ] Implement `ContainerTestHarness` struct with methods:
  - [ ] `build_image(universal_build: &UniversalBuild) -> Result<ImageId>`
  - [ ] `start_container(image_id: &ImageId) -> Result<Container>`
  - [ ] `wait_for_port(container: &Container, port: u16, timeout: Duration) -> Result<()>`
  - [ ] `http_health_check(container: &Container, port: u16, path: &str) -> Result<bool>`
- [ ] Add timeout handling (default: 30s for container start, 10s for health check)
- [ ] Add cleanup logic to stop and remove containers after tests
- [ ] Add unit tests for harness helper methods

## Track 2: Enumerate Test Fixtures (~50 LOC)

### Task 3: Analyze Existing Fixtures
- [ ] Review all 17 single-language fixtures in `tests/fixtures/single-language/`
- [ ] Review all 6 monorepo fixtures in `tests/fixtures/monorepo/`
- [ ] For each fixture, determine:
  - [ ] Expected port(s) the application should listen on
  - [ ] Health check path(s) (e.g., `/health`, `/actuator/health`)
  - [ ] Whether fixture needs modification to expose health endpoint
- [ ] Create fixture analysis document listing all testable fixtures
- [ ] Identify fixtures that cannot be tested (e.g., libraries with no server)

## Track 3: Integration Tests (~150 LOC)

### Task 4: Extend e2e.rs with Container Integration Tests (~150 LOC)
- [ ] Add to existing `tests/e2e.rs` file
- [ ] Define `ContainerTestCase` struct with fields:
  - [ ] `fixture_name: &str` (e.g., "rust-cargo")
  - [ ] `fixture_path: &str` (e.g., "tests/fixtures/single-language/rust-cargo")
  - [ ] `expected_port: u16` (e.g., 3000)
  - [ ] `health_path: &str` (default: "/health")
- [ ] Create const array `CONTAINER_TEST_CASES: &[ContainerTestCase]` with all testable fixtures
- [ ] Implement helper function `run_container_integration_test(test_case, detection_mode)`
- [ ] Add parameterized test function:
  ```rust
  #[test]
  fn test_container_integration() {
      for test_case in CONTAINER_TEST_CASES {
          for mode in &[DetectionMode::Static, DetectionMode::Full] {
              run_container_integration_test(test_case, *mode)?;
          }
      }
  }
  ```
- [ ] Each test iteration:
  - [ ] Run aipack detection with specified mode
  - [ ] Build container image using ContainerTestHarness
  - [ ] Start container
  - [ ] Wait for port to be accessible
  - [ ] Make HTTP GET request to health_path
  - [ ] Assert response is 200 OK
  - [ ] Cleanup container

This approach reuses all existing fixtures and tests both modes:
- ~17-23 testable fixtures (single-language + monorepo) × 2 modes = ~34-46 iterations
- Tests run automatically as part of normal `cargo test --test e2e` suite

## Track 4: Documentation (~20 LOC)

### Task 5: Refactor Existing e2e.rs to Use Parameterized Tests (~100 LOC reduction)
- [ ] Add `parameterized = "2.1.0"` to Cargo.toml dev-dependencies
- [ ] Define test case data structures for existing tests
- [ ] Convert single-language fixture tests to parameterized approach
- [ ] Convert monorepo fixture tests to parameterized approach
- [ ] Convert LLM mode tests to parameterized approach
- [ ] Convert static mode tests to parameterized approach
- [ ] Remove copypaste test functions (reduce from ~73 functions to ~4-5 parameterized tests)
- [ ] Verify all tests still pass after refactoring

### Task 6: Documentation (~20 LOC)
- [ ] Add integration test section to tests/README.md
- [ ] Document how to run: `cargo test --test e2e`
- [ ] Document Docker requirements (version, BuildKit)
- [ ] Add troubleshooting section for common failures
- [ ] Note about BuildKit layer caching improving speed

## Validation

- [ ] All container integration test iterations pass locally with Docker running
- [ ] Monorepo fixtures produce multiple UniversalBuild outputs, each validated separately
- [ ] Tests properly cleanup containers on failure
- [ ] Parameterized test approach works correctly
- [ ] BuildKit layer caching makes subsequent runs fast
- [ ] Documentation is clear and complete

## Success Criteria

- ✅ Integration tests validate that generated UniversalBuild specs produce working containers
- ✅ Both static and LLM detection modes are tested end-to-end
- ✅ Tests run automatically as part of `cargo test` suite
- ✅ Parameterized test approach minimizes code duplication
- ✅ Tests fail fast when containers don't start or health checks fail
- ✅ BuildKit layer caching makes tests fast on subsequent runs
- ✅ Clear error messages help debug container build/runtime failures
