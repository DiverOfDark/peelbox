# Spec Delta: CLI Mode Control

## ADDED Requirements

### Requirement: Detection Mode Environment Variable
The CLI MUST support `PEELBOX_DETECTION_MODE` environment variable to control whether detection uses LLM, static analysis only, or both.

#### Scenario: Parsing detection mode from environment
**Given** the peelbox CLI is invoked
**When** `PEELBOX_DETECTION_MODE` environment variable is set
**Then** the CLI parses the value (case-insensitive)
**And** maps "static" to DetectionMode::StaticOnly
**And** maps "llm" to DetectionMode::LLMOnly
**And** maps "full" to DetectionMode::Full
**And** defaults to DetectionMode::Full if not set or invalid value

#### Scenario: Static mode skips LLM initialization
**Given** the CLI is invoked with `PEELBOX_DETECTION_MODE=static`
**When** detection service is created
**Then** no real LLM client is initialized
**And** a NoOpLLMClient is used instead
**And** the NoOpLLMClient returns error if any phase calls it

### Requirement: Pipeline Mode Propagation
The detection mode MUST be passed from CLI through DetectionService to PipelineOrchestrator and all pipeline phases.

#### Scenario: Mode propagation through pipeline
**Given** the CLI sets DetectionMode based on environment variable
**When** detection is executed
**Then** the mode is passed to DetectionService
**And** DetectionService passes mode to PipelineOrchestrator
**And** PipelineOrchestrator passes mode to each phase that uses LLM
**And** phases respect the mode when deciding whether to call LLM

### Requirement: NoOpLLMClient Implementation
A NoOpLLMClient MUST be provided that returns an error if called, for use in static-only mode.

#### Scenario: NoOpLLMClient returns error on call
**Given** a NoOpLLMClient instance
**When** any method is called (e.g., chat())
**Then** it returns an error indicating LLM was called in static mode
**And** the error message helps developers identify which phase made the incorrect call

#### Scenario: NoOpLLMClient used in static mode
**Given** the CLI runs with `PEELBOX_DETECTION_MODE=static`
**When** detection service is created
**Then** it uses NoOpLLMClient instead of real LLM client
**And** if any phase incorrectly calls LLM, detection fails with clear error

### Requirement: Phase Static Mode Handling
All pipeline phases that use LLM MUST check detection mode and skip LLM calls in static-only mode.

#### Scenario: Phase respects static mode
**Given** a pipeline phase that can use LLM (e.g., classify, structure, dependencies)
**When** the phase executes with DetectionMode::StaticOnly
**Then** the phase tries deterministic/static analysis first
**And** if deterministic analysis succeeds, returns the result
**And** if deterministic analysis fails or has low confidence, still returns best-effort result
**And** the phase does NOT call LLM client

#### Scenario: Phase uses LLM in LLM mode
**Given** a pipeline phase that can use LLM
**When** the phase executes with DetectionMode::LLMOnly or DetectionMode::Full
**And** deterministic analysis is insufficient
**Then** the phase calls LLM client for additional analysis
**And** returns enhanced result from LLM

## ADDED Requirements

### Requirement: E2e Tests Use Real Filesystem
E2e tests MUST continue using real filesystem to validate the full CLI binary behavior.

#### Scenario: E2e tests access real fixtures
**Given** an e2e test in `tests/e2e.rs`
**When** the test spawns the peelbox binary
**Then** the binary accesses fixture files from real filesystem
**And** the test validates that CLI correctly reads repository structure
**And** no MockFileSystem is used in e2e tests
