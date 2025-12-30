# aipack Test Suite

This directory contains the test suite for aipack, including unit tests, end-to-end tests, and integration tests.

## Test Types

### 1. Unit Tests (`cargo test --lib`)
Located in `src/**/*.rs` files. Fast, deterministic tests that verify individual components.

**Run all library tests:**
```bash
cargo test --lib
```

### 2. End-to-End Tests (`tests/e2e.rs`)
Tests the complete detection pipeline using fixture repositories.

**Run all e2e tests:**
```bash
cargo test --test e2e
```

**Run specific fixture test:**
```bash
cargo test --test e2e test_single_language::rust_cargo_detection
```

**Test modes:**
- **Full mode** (default): LLM + static analysis
- **Static mode**: Static analysis only (fast, no LLM)
  ```bash
  cargo test --test e2e static
  ```
- **LLM mode**: LLM-only detection
  ```bash
  cargo test --test e2e llm
  ```

### 3. Container Integration Tests (e2e.rs)
Tests that verify generated UniversalBuild specs produce working containers:
1. Run detection to generate UniversalBuild JSON
2. Build container image using BuildKit
3. Start container and verify it responds to HTTP requests
4. Perform health checks on declared endpoints

**Requirements:**
- Docker must be installed and running
- BuildKit support (enabled by default in Docker 23.0+)
- buildctl CLI tool available in PATH

**Run container integration tests:**
```bash
# Run all container integration tests
cargo test --test e2e test_container_integration

# Run static mode container tests (faster, no LLM)
cargo test --test e2e test_container_integration_single_language

# Run full mode container tests (with LLM fallback)
cargo test --test e2e test_container_integration_single_language_full
```

**Testable fixtures:**
- `rust-cargo` (Actix Web, port 8080, /health)
- `go-mod` (Gin, port 8080, /health)
- `python-pip` (Flask, port 5000, /health)
- `python-poetry` (Flask, port 5000, /health)
- `node-npm` (Express, port 3000, /health)
- `ruby-bundler` (Sinatra, port 4567, /health)
- `java-maven` (Spring Boot, port 8080, /actuator/health)
- `java-gradle` (Spring Boot, port 8080, /actuator/health)
- `dotnet-csproj` (ASP.NET Core, port 5000, /health)
- `php-symfony` (Symfony, port 8000, /_health)

**What it validates:**
1. UniversalBuild spec is syntactically correct
2. LLB generation succeeds
3. Container image builds without errors
4. Container starts successfully
5. Application listens on declared port
6. Health check endpoint returns 200 OK
7. Both static and full detection modes work

**Performance:**
- First run: ~2-5 minutes per fixture (downloads base images, builds from scratch)
- Subsequent runs: ~10-30 seconds per fixture (BuildKit layer caching)
- Static mode tests: Faster (no LLM inference)
- **Parallel execution: Enabled** - Tests use:
  - Dynamic port allocation (no port conflicts)
  - Shared BuildKit container (single instance for all parallel builds)
  - Concurrent image builds (BuildKit handles parallel requests)

**Note:** BuildKit layer caching significantly speeds up subsequent runs. The cache persists across test runs in a Docker volume named `buildkit-cache`. A single shared BuildKit container handles all parallel builds, avoiding container startup overhead and lock conflicts. Dynamic port allocation prevents port conflicts between parallel test containers.

### 4. BuildKit Integration Test (`tests/buildkit_integration.rs`)
End-to-end test that verifies the complete BuildKit frontend workflow by building aipack itself:
1. Generate LLB using aipack frontend
2. Build container image using BuildKit
3. Run the built image and verify output

**Requirements:**
- Docker must be installed and running
- BuildKit support (enabled by default in Docker 23.0+)

**Run the integration test:**
```bash
cargo test --test buildkit_integration -- --nocapture
```

**What it tests:**
1. Image builds successfully and exists in registry
2. Built image runs and outputs help text correctly
3. Distroless layer structure (no wolfi-base in history)
4. Image size is optimized (< 200MB)
5. Binary exists at /usr/local/bin/aipack and is executable
6. Various buildctl output types (OCI and Docker tarballs)

**Note:** This test requires Docker or Podman to be installed and running. If Docker is not available, the test will fail with a connection error.

## Test Fixtures

Test fixtures are located in `tests/fixtures/`:
- `single-language/` - Single build system projects
- `monorepo/` - Monorepo/workspace projects
- `edge-cases/` - Edge cases and unusual configurations
- `expected/` - Expected JSON outputs for validation

Each fixture directory contains a minimal project structure representing a specific language/build system combination.

## LLM Recording System

The e2e tests use an LLM recording system for deterministic testing:

**Recording modes** (controlled via `AIPACK_RECORDING_MODE` environment variable):
- `record` - Make live LLM calls and save responses
- `replay` - Use saved responses (fails if recording missing)
- `auto` (default) - Replay if available, otherwise record

**Update recordings after prompt changes:**
```bash
# Delete old recordings
rm -rf tests/recordings/

# Re-record with live LLM
AIPACK_RECORDING_MODE=record cargo test --test e2e

# Verify new recordings
AIPACK_RECORDING_MODE=replay cargo test --test e2e

# Commit updated recordings
git add tests/recordings/
git commit -m "chore: Update LLM recordings after prompt changes"
```

## Running All Tests

Run the complete test suite (excluding integration tests):
```bash
cargo test
```

Run all tests including integration tests (requires Docker):
```bash
cargo test --workspace -- --include-ignored
```

## Continuous Integration

The CI pipeline runs:
1. Library tests (`cargo test --lib`)
2. E2E tests in static mode (`AIPACK_DETECTION_MODE=static cargo test --test e2e`)
3. LLM recording validation (`AIPACK_RECORDING_MODE=replay cargo test --test e2e`)

BuildKit integration tests are run manually or on-demand due to Docker requirements.
