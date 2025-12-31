# testing Specification Changes

## ADDED Requirements

### Requirement: Mandatory Health Endpoints for Container Tests
All test fixtures SHALL include working HTTP servers with health endpoints for integration testing.

#### Scenario: Health endpoint required
- **WHEN** running container integration test (e2e.rs)
- **THEN** the test fixture MUST have a `health` field in universalbuild.json
- **AND** the test asserts health endpoint exists (no optional health checks)
- **AND** the test fails if health endpoint is missing

#### Scenario: HTTP health check success
- **WHEN** container is running from test fixture
- **THEN** the test performs HTTP GET to health endpoint
- **AND** expects 200 OK response within 10 seconds
- **AND** fails the test if health check times out or returns non-200

#### Scenario: No skipping container tests
- **WHEN** implementing test fixtures
- **THEN** container integration tests are NEVER skipped
- **AND** all fixtures implement working HTTP servers
- **AND** fixtures without HTTP servers are updated to include them

---

### Requirement: Static HTML Test Fixture
The test suite SHALL include a static HTML fixture served by nginx.

#### Scenario: Static HTML fixture structure
- **WHEN** static-html fixture exists in `tests/fixtures/single-language/static-html/`
- **THEN** it contains `index.html` and `assets/style.css`
- **AND** `universalbuild.json` specifies nginx runtime
- **AND** health endpoint is `/`

#### Scenario: Static HTML e2e tests
- **WHEN** running e2e tests for static-html
- **THEN** detection test verifies UniversalBuild specification is correct
- **AND** container test builds image, runs nginx, and checks health endpoint
- **AND** both tests pass successfully

---

### Requirement: Dockerfile Compatibility Test Fixture
The test suite SHALL include a fixture demonstrating Dockerfile compatibility via docker/dockerfile:1 frontend.

#### Scenario: Dockerfile fixture structure
- **WHEN** dockerfile-exists fixture exists in `tests/fixtures/edge-cases/dockerfile-exists/`
- **THEN** it contains a Dockerfile, package.json, index.js
- **AND** `universalbuild.json` specifies expected detection output
- **AND** health endpoint is defined

#### Scenario: Dockerfile frontend delegation
- **WHEN** Dockerfile is detected in repository root
- **THEN** peelbox delegates LLB generation to `docker/dockerfile:1` frontend
- **AND** does NOT parse Dockerfile syntax itself
- **AND** supports all Dockerfile features (multi-stage, buildx, etc.)

#### Scenario: Dockerfile e2e tests
- **WHEN** running e2e tests for dockerfile-exists
- **THEN** detection test verifies Dockerfile is detected
- **AND** LLB generation test verifies delegation to docker/dockerfile:1 frontend
- **AND** container test builds image and checks health endpoint

---

### Requirement: HTTP Servers for All Fixtures
All language fixtures SHALL implement HTTP servers with health endpoints for container testing.

#### Scenario: C++ fixture with HTTP server
- **WHEN** cpp-cmake fixture is tested
- **THEN** the fixture includes HTTP server using Crow or httplib
- **AND** implements `/health` endpoint returning 200 OK
- **AND** container test verifies health endpoint

#### Scenario: Elixir fixture with HTTP server
- **WHEN** elixir-mix fixture is tested
- **THEN** the fixture includes HTTP server using Plug
- **AND** implements `/health` endpoint returning 200 OK
- **AND** container test verifies health endpoint

---

## MODIFIED Requirements

### Requirement: Test Fixture Health Checks
E2e tests SHALL enforce health endpoint requirements for all fixtures.

#### Scenario: Health check enforcement (previously optional)
- **WHEN** running container integration test
- **THEN** the test expects `spec.runtime.health` to be present (not optional)
- **AND** fails immediately if health field is missing
- **AND** does NOT skip health check with `if let Some(health_endpoint)`

---

## REMOVED Requirements

None - This change adds new fixtures and enforces existing health check requirements more strictly.
