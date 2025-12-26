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

### 3. BuildKit Integration Test (`tests/buildkit_integration.rs`)
End-to-end test that verifies the complete BuildKit frontend workflow:
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

**What it does:**
1. Starts a BuildKit container (`moby/buildkit:latest`)
2. Builds aipack binary in release mode (if not already built)
3. Runs `aipack frontend` to generate LLB from `universalbuild.json`
4. Pipes LLB to `buildctl build` inside the BuildKit container
5. Exports the built image to local Docker
6. Runs the image with `--help` flag
7. Verifies the output contains expected help text
8. Cleans up containers and images

**Expected output:**
```
=== BuildKit Integration Test ===
✓ Docker is available
✓ BuildKit container running
✓ aipack binary available
✓ Generated LLB: 1234 bytes
✓ Image built successfully
✓ Image loaded into Docker
✓ Image exists in Docker
✓ Help output is valid
✓ BuildKit container removed
✓ Test image removed
=== ✓ BuildKit Integration Test PASSED ===
```

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
