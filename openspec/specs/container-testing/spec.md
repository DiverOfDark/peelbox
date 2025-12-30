# container-testing Specification

## Purpose
TBD - created by archiving change add-container-integration-tests. Update Purpose after archive.
## Requirements
### Requirement: Container Test Harness

The system SHALL provide a `ContainerTestHarness` that builds container images from UniversalBuild specs and validates runtime behavior. The harness MUST support building images via buildkit-client, starting containers, waiting for port availability, and performing HTTP health checks.

**Rationale**: Current e2e tests only validate JSON output, not whether the generated build specifications actually produce working containers. A test harness bridges the gap between detection output and runtime validation.

#### Scenario: Build image from UniversalBuild

```rust
let harness = ContainerTestHarness::new()?;
let universal_build = detect_service("tests/fixtures/integration/node-express")?;
let image_id = harness.build_image(&universal_build).await?;
assert!(image_id.len() > 0);
```

The harness must use buildkit-client to translate UniversalBuild to LLB graph and build the image.

#### Scenario: Start container and verify port

```rust
let container = harness.start_container(&image_id).await?;
harness.wait_for_port(&container, 3000, Duration::from_secs(30)).await?;
assert!(container.is_running());
```

The harness must start the container, wait for the declared port to become accessible, and fail if timeout is exceeded.

#### Scenario: HTTP health check validation

```rust
let healthy = harness.http_health_check(&container, 3000, "/health").await?;
assert!(healthy, "Container should respond 200 OK on /health");
```

The harness must make an HTTP GET request to the specified path and verify a 200 OK response.

#### Scenario: Automatic cleanup on test completion

```rust
{
    let harness = ContainerTestHarness::new()?;
    let container = harness.start_container(&image_id).await?;
    // test logic
} // harness dropped, container automatically stopped and removed
```

The harness must implement Drop to cleanup containers even if tests panic or fail.

---

### Requirement: Integration Test Fixtures

The system SHALL reuse existing test fixtures from `tests/fixtures/single-language/` (17 fixtures) and `tests/fixtures/monorepo/` (6 fixtures) for container integration testing. Fixtures that can expose HTTP endpoints MUST be analyzed to determine expected ports and health check paths. The test suite MUST identify which fixtures are testable (server applications) versus non-testable (libraries, build-only projects).

**Rationale**: The project already has comprehensive test fixtures covering all supported languages and build systems. Reusing these fixtures validates the full detection-to-container pipeline without duplicating test data. Not all fixtures represent runnable server applications - some are libraries or build-only projects that cannot be health-checked.

#### Scenario: Reuse existing single-language fixtures

```
tests/fixtures/single-language/
├── rust-cargo/           # Existing Rust fixture
├── node-npm/             # Existing Node.js fixture
├── python-pip/           # Existing Python fixture
├── java-maven/           # Existing Java fixture
├── go-mod/               # Existing Go fixture
└── ... (12 more)
```

The test suite must:
- Enumerate all 17 single-language fixtures
- For each testable fixture, determine expected port and health path
- Skip fixtures that are libraries or cannot expose HTTP endpoints

#### Scenario: Reuse existing monorepo fixtures

```
tests/fixtures/monorepo/
├── npm-workspaces/       # Existing npm workspaces
├── turborepo/            # Existing Turborepo
├── cargo-workspace/      # Existing Cargo workspace
├── gradle-multiproject/  # Existing Gradle multiproject
├── maven-multimodule/    # Existing Maven multimodule
└── polyglot/             # Existing polyglot monorepo
```

The test suite must:
- Enumerate all 6 monorepo fixtures
- For each testable service in the monorepo, determine expected port and health path
- Validate that monorepo detection produces multiple UniversalBuild outputs
- Test each service independently with container builds

---

### Requirement: Dual-Mode Integration Testing

The test suite SHALL validate both static and LLM detection modes by running integration tests for each mode. Tests MUST verify that both modes produce UniversalBuild specs that result in working, health-check-passing containers.

**Rationale**: Static mode uses parsers, LLM mode uses inference. Both must produce functionally equivalent UniversalBuild specs that result in working containers.

#### Scenario: Static mode integration test

```rust
#[test]
#[ignore] // Requires Docker
fn test_node_express_static_integration() {
    std::env::set_var("AIPACK_DETECTION_MODE", "static");

    let harness = ContainerTestHarness::new()?;
    let build = detect_service("tests/fixtures/integration/node-express")?;
    let image = harness.build_image(&build).await?;
    let container = harness.start_container(&image).await?;

    harness.wait_for_port(&container, 3000, Duration::from_secs(30)).await?;
    let healthy = harness.http_health_check(&container, 3000, "/health").await?;

    assert!(healthy);
}
```

The test must use only deterministic parsing, no LLM calls.

#### Scenario: LLM mode integration test

```rust
#[test]
#[ignore] // Requires Docker and LLM
fn test_node_express_llm_integration() {
    std::env::set_var("AIPACK_DETECTION_MODE", "full");

    let harness = ContainerTestHarness::new()?;
    let build = detect_service("tests/fixtures/integration/node-express")?;
    let image = harness.build_image(&build).await?;
    let container = harness.start_container(&image).await?;

    harness.wait_for_port(&container, 3000, Duration::from_secs(30)).await?;
    let healthy = harness.http_health_check(&container, 3000, "/health").await?;

    assert!(healthy);
}
```

The test must allow LLM fallbacks, validating end-to-end LLM-assisted detection.

#### Scenario: Failure isolation between modes

```rust
// If static mode test fails, LLM mode test should still run
// Tests must be independent and not share state
```

Each test must create its own harness, build separate images, and cleanup independently.

---

### Requirement: Parameterized Test Suite

Integration tests SHALL extend the existing `tests/e2e.rs` file with a parameterized test approach that iterates over test cases, reducing code duplication. Tests MUST be integrated into the existing e2e test suite and run automatically with `cargo test --test e2e`.

**Rationale**: Writing separate test functions for each fixture and detection mode leads to significant duplication. A parameterized approach with const test case definitions is more maintainable and easier to extend. Extending the existing e2e.rs file keeps all end-to-end tests in one place.

#### Scenario: Define test cases declaratively in e2e.rs

```rust
struct ContainerTestCase {
    fixture_name: &'static str,
    fixture_path: &'static str,
    expected_port: u16,
    health_path: &'static str,
}

const CONTAINER_TEST_CASES: &[ContainerTestCase] = &[
    ContainerTestCase {
        fixture_name: "rust-cargo",
        fixture_path: "tests/fixtures/single-language/rust-cargo",
        expected_port: 3000,
        health_path: "/health",
    },
    ContainerTestCase {
        fixture_name: "node-npm",
        fixture_path: "tests/fixtures/single-language/node-npm",
        expected_port: 3000,
        health_path: "/health",
    },
    // ... all testable fixtures from tests/fixtures/
];
```

Test cases must be defined as const data, not code.

#### Scenario: Add parameterized test to e2e.rs

```rust
#[test]
fn test_container_integration() {
    for test_case in CONTAINER_TEST_CASES {
        for mode in &[DetectionMode::Static, DetectionMode::Full] {
            println!("Testing {} in {:?} mode", test_case.fixture_name, mode);
            run_container_integration_test(test_case, *mode).expect("Integration test failed");
        }
    }
}
```

The test must iterate over all cases and modes, reporting failures clearly.

---

### Requirement: Refactor Existing Tests to Use Parameterized Approach

The existing `tests/e2e.rs` file SHALL be refactored to use the `parameterized` crate, eliminating code duplication across the ~73 existing test functions. The refactoring MUST use a declarative test case approach with const arrays and MUST maintain full test coverage.

**Rationale**: The current e2e.rs has significant copypaste across test functions - each fixture (single-language, monorepo, llm, static) has nearly identical test logic with only the fixture name and expected build system varying. Using the `parameterized` crate reduces ~73 test functions to ~4-5 parameterized tests, making the test suite more maintainable.

#### Scenario: Define test cases declaratively

```rust
use parameterized::parameterized;

struct E2eTestCase {
    category: &'static str,
    fixture_name: &'static str,
    expected_build_system: &'static str,
}

const SINGLE_LANGUAGE_CASES: &[E2eTestCase] = &[
    E2eTestCase { category: "single-language", fixture_name: "rust-cargo", expected_build_system: "Cargo" },
    E2eTestCase { category: "single-language", fixture_name: "node-npm", expected_build_system: "npm" },
    E2eTestCase { category: "single-language", fixture_name: "python-pip", expected_build_system: "pip" },
    // ... all 17 single-language fixtures
];

const MONOREPO_CASES: &[E2eTestCase] = &[
    E2eTestCase { category: "monorepo", fixture_name: "npm-workspaces", expected_build_system: "npm" },
    E2eTestCase { category: "monorepo", fixture_name: "turborepo", expected_build_system: "npm" },
    // ... all 6 monorepo fixtures
];
```

#### Scenario: Parameterized test replaces 17+ copypaste functions

```rust
#[parameterized(case = { SINGLE_LANGUAGE_CASES })]
#[serial]
fn test_single_language_detection(case: E2eTestCase) {
    let fixture = fixture_path(case.category, case.fixture_name);
    let test_name = format!("e2e_test_{}_detection", case.fixture_name.replace("-", "_"));
    let results = run_detection(fixture, &test_name).expect("Detection failed");
    assert_detection(&results, case.expected_build_system, case.fixture_name);
}
```

The refactoring must eliminate the 17+ nearly-identical `test_*_detection()` functions.

#### Scenario: Support mode variants (llm, static)

```rust
#[parameterized(case = { SINGLE_LANGUAGE_CASES })]
#[serial]
fn test_single_language_llm(case: E2eTestCase) {
    let fixture = fixture_path(case.category, case.fixture_name);
    let test_name = format!("e2e_test_{}_llm", case.fixture_name.replace("-", "_"));
    let results = run_detection_llm(fixture, &test_name).expect("Detection failed");
    assert_detection(&results, case.expected_build_system, case.fixture_name);
}

#[parameterized(case = { SINGLE_LANGUAGE_CASES })]
#[serial]
fn test_single_language_static(case: E2eTestCase) {
    let fixture = fixture_path(case.category, case.fixture_name);
    let test_name = format!("e2e_test_{}_static", case.fixture_name.replace("-", "_"));
    let results = run_detection_static(fixture, &test_name).expect("Detection failed");
    assert_detection_with_mode(&results, case.expected_build_system, case.fixture_name, Some("static"));
}
```

The refactoring must support all three modes (default, llm, static) with parameterized tests.

---

### Requirement: BuildKit Layer Caching

Integration tests MUST leverage BuildKit's layer caching to achieve fast execution on subsequent runs. Initial test runs may take minutes; cached runs should complete in seconds.

**Rationale**: Building containers from scratch is slow (minutes per fixture). BuildKit's layer caching means only changed layers rebuild, making subsequent test runs as fast as unit tests.

#### Scenario: Fast cached builds

```rust
// First run: build from scratch
$ cargo test --test container_integration
...
test container_integration_tests ... ok (120s)

// Second run: layers cached
$ cargo test --test container_integration
...
test container_integration_tests ... ok (8s)
```

The harness must use BuildKit with caching enabled, not disable caching.

#### Scenario: Cache key based on UniversalBuild content

```rust
let cache_key = sha256(&serde_json::to_string(&universal_build)?);
let image_tag = format!("aipack-test:{}", cache_key);
```

The harness should use content-addressed image tags to maximize cache hits.

