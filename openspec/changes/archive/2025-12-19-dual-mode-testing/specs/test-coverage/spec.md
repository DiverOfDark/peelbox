# Spec Delta: Test Coverage

## ADDED Requirements

### Requirement: Dual-Mode E2e Testing
The test suite MUST support running each e2e test in both LLM mode and static-only mode by spawning the CLI binary with different environment variables.

#### Scenario: Running LLM mode e2e tests
**Given** a test fixture for a supported language/build system
**When** the test spawns the CLI with `PEELBOX_DETECTION_MODE=llm`
**Then** the binary executes with LLM-only detection mode
**And** the pipeline uses LLM for all phases that need it
**And** the binary outputs valid JSON matching expected output

#### Scenario: Running static-only mode e2e tests
**Given** a test fixture for a supported language/build system
**When** the test spawns the CLI with `PEELBOX_DETECTION_MODE=static`
**Then** the binary executes with static-only detection mode
**And** the pipeline completes without any LLM calls
**And** the binary outputs valid JSON using only deterministic analysis
**And** if LLM is accidentally called, the binary returns an error

#### Scenario: Running full mode e2e tests
**Given** a test fixture for a supported language/build system
**When** the test spawns the CLI without PEELBOX_DETECTION_MODE (defaults to "full")
**Then** the binary tries static analysis first, then falls back to LLM if needed
**And** the binary outputs valid JSON

### Requirement: Complete Fixture Coverage
Every test fixture MUST have corresponding LLM and static mode e2e test variants.

#### Scenario: Single-language fixtures
**Given** 17 single-language fixtures (rust-cargo, node-npm, python-pip, etc.)
**When** running the e2e test suite
**Then** each fixture has both `test_{name}_llm()` and `test_{name}_static()` tests
**And** all 34 tests (17 × 2) pass successfully
**And** all tests spawn the CLI binary

#### Scenario: Monorepo fixtures
**Given** 7 monorepo fixtures (cargo-workspace, npm-workspaces, etc.)
**When** running the e2e test suite
**Then** each fixture has both LLM and static mode tests
**And** all 14 tests (7 × 2) pass successfully
**And** all tests spawn the CLI binary

#### Scenario: Edge case fixtures
**Given** edge case fixtures (empty-repo, no-manifest, etc.)
**When** running the e2e test suite
**Then** each fixture has appropriate tests for both modes
**And** tests correctly handle expected failures (e.g., empty repo fails detection)
**And** all tests spawn the CLI binary

### Requirement: CLI Mode Control
The CLI MUST respect the `PEELBOX_DETECTION_MODE` environment variable to control detection mode.

#### Scenario: Environment variable parsing
**Given** the peelbox binary is executed
**When** `PEELBOX_DETECTION_MODE` is set to "static"
**Then** the binary uses DetectionMode::StaticOnly
**When** `PEELBOX_DETECTION_MODE` is set to "llm"
**Then** the binary uses DetectionMode::LLMOnly
**When** `PEELBOX_DETECTION_MODE` is set to "full" or not set
**Then** the binary uses DetectionMode::Full (default)

#### Scenario: Static mode without LLM backend
**Given** the binary is executed with `PEELBOX_DETECTION_MODE=static`
**When** detection runs
**Then** no LLM client is initialized (or NoOpLLMClient is used)
**And** detection completes successfully using only deterministic paths
**And** if any phase calls LLM, the binary returns an error

### Requirement: Deterministic Test Execution
E2e tests in static mode MUST produce consistent results across runs without external dependencies.

#### Scenario: Static mode tests without LLM backend
**Given** a CI environment without LLM access
**When** running e2e tests with `PEELBOX_DETECTION_MODE=static`
**Then** all static mode tests pass without requiring LLM backend
**And** test execution completes in < 10 seconds for all static tests

#### Scenario: LLM mode tests with embedded model
**Given** e2e tests run with `PEELBOX_DETECTION_MODE=llm`
**When** using embedded model via `PEELBOX_PROVIDER=embedded`
**Then** tests use deterministic embedded LLM responses
**And** test results are consistent across runs

## ADDED Requirements

### Requirement: E2e Test Structure
All dual-mode tests MUST remain as e2e tests that spawn the CLI binary and validate JSON output.

#### Scenario: E2e test execution
**Given** an e2e test for any fixture
**When** the test runs
**Then** it spawns the peelbox binary using Command::new()
**And** passes PEELBOX_DETECTION_MODE environment variable
**And** reads JSON output from binary stdout
**And** validates the output against expected JSON
**And** uses real filesystem for fixture access
