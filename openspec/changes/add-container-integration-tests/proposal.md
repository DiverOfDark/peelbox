# Change: Add Container Integration Tests

## Why

Currently, aipack's e2e tests verify that detection produces correct JSON output (UniversalBuild schema), but they don't validate that the generated build specifications actually work. We have no automated way to verify that:

1. **Images Build Successfully** - The generated UniversalBuild can be translated into a working container build
2. **Containers Start** - The built image runs without crashing
3. **Services Respond** - The application listens on the declared port and responds to HTTP requests
4. **Both Modes Work** - Both LLM and static detection modes produce working containers

This gap means we could ship detection logic that generates syntactically correct but functionally broken build specifications. Integration tests would catch issues like:
- Incorrect entrypoints that fail to start
- Missing dependencies that cause runtime crashes
- Wrong port configurations that prevent service access
- Build commands that fail in the actual container environment

## What Changes

Extend existing e2e tests to:

1. **Build Real Containers** - Use buildkit-wolfi-frontend to build actual container images from generated UniversalBuild specs
2. **Start Test Containers** - Use testcontainers-rs to start the built images
3. **Validate Service Health** - Make HTTP requests to the declared port and verify 200 OK responses
4. **Reuse Existing Fixtures** - All 17 single-language and 6 monorepo fixtures in tests/fixtures/

Integration tests run automatically in the normal test suite. BuildKit layer caching ensures subsequent runs are fast (seconds, not minutes).

## Dependencies

- **Requires**: `add-buildkit-wolfi-frontend` (for building images from UniversalBuild)
- **Blocks**: None (this is purely additive testing infrastructure)

## Scope

- Add testcontainers-rs dependency
- Create ContainerTestHarness for building and running containers
- Add integration test fixtures (minimal working apps for each language)
- Implement container_integration_tests.rs test suite with parameterized tests
- Tests run automatically in `cargo test` (skip gracefully if Docker unavailable)

## Out of Scope

- Complex application scenarios (focus on "hello world" level validation)
- Performance testing or load testing
- Multi-container scenarios with external dependencies (databases, message queues, etc.)
