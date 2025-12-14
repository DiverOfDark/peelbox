# Spec: Prompt Pipeline Architecture

## ADDED Requirements

### Requirement: Pipeline Orchestration

The system SHALL execute repository analysis as a multi-phase pipeline where code orchestrates the workflow and LLM calls are minimal and single-purpose.

#### Scenario: Single-service repository detection

**Given** a repository containing a single Node.js service
**When** the pipeline executes
**Then** it SHALL complete phases 1-3, 6a-6g, 7, and 9
**And** it SHALL skip phase 4 (using deterministic parser)
**And** it SHALL skip phase 8 (not a monorepo)
**And** it SHALL use <3000 tokens total

#### Scenario: Monorepo with multiple services

**Given** a pnpm monorepo with 3 services (apps/web, apps/api, libs/shared)
**When** the pipeline executes
**Then** it SHALL complete all 10 phases
**And** phase 6 SHALL analyze all 3 services in parallel
**And** each service SHALL have 6a-6g run concurrently (7 parallel LLM calls)
**And** it SHALL use <5000 tokens total

#### Scenario: Unknown manifest format

**Given** a repository with an unsupported build manifest (e.g., `build.xml` for Ant)
**When** phase 4 executes
**Then** it SHALL fall back to LLM-based dependency extraction
**And** the result SHALL indicate `detected_by: DetectionMethod::LLM`
**And** confidence SHALL be `medium` or `low`

### Requirement: Deterministic Parsing

The system SHALL use deterministic parsers for known manifest formats, bypassing LLM calls when possible.

#### Scenario: Node.js package.json parsing

**Given** a repository with a valid `package.json`
**When** phase 4 executes
**Then** it SHALL extract dependencies using `NodeParser`
**And** it SHALL NOT make an LLM call
**And** the result SHALL indicate `detected_by: DetectionMethod::Deterministic`
**And** confidence SHALL be `high`

#### Scenario: Rust Cargo.toml parsing

**Given** a Cargo workspace with multiple members
**When** phase 4 executes
**Then** it SHALL parse `Cargo.toml` to extract workspace members
**And** it SHALL identify internal dependencies between members
**And** it SHALL NOT make an LLM call

#### Scenario: Supported manifest formats

**Given** the system is analyzing a repository
**Then** it SHALL support deterministic parsing for:
- `package.json` (Node.js)
- `pnpm-workspace.yaml` (pnpm monorepos)
- `Cargo.toml` (Rust)
- `go.mod` (Go)
- `pom.xml` (Maven)
- `build.gradle`, `build.gradle.kts` (Gradle)
- `pyproject.toml`, `requirements.txt` (Python)

### Requirement: Parallel Execution

The system SHALL execute independent phases in parallel to minimize latency.

#### Scenario: Service analysis parallelization

**Given** a monorepo with 3 services
**When** phase 6 executes
**Then** all 3 services SHALL be analyzed concurrently
**And** within each service, phases 6a-6g SHALL run concurrently
**And** total execution time SHALL be ≤ max(service_analysis_time) + sequential_phase_time

#### Scenario: Sequential dependency preservation

**Given** the pipeline is executing
**When** phases 1-5 complete
**Then** phase 6 SHALL NOT start until phase 5 completes
**And** phase 7 SHALL NOT start until all phase 6 analyses complete

### Requirement: Minimal Context Prompts

The system SHALL ensure each LLM prompt fits within 8k tokens and contains only essential context.

#### Scenario: Runtime detection prompt size

**Given** a service with 500+ files
**When** phase 6a (runtime detection) builds its prompt
**Then** the prompt SHALL include ≤20 relevant filenames
**And** the prompt SHALL include ≤500 characters of manifest excerpt
**And** the total prompt SHALL be ≤200 tokens

#### Scenario: Port discovery prompt size

**Given** a service with environment files, config files, and source code
**When** phase 6e (port discovery) builds its prompt
**Then** the prompt SHALL include only extracted port sources (not full files)
**And** each source SHALL be ≤100 characters
**And** the total prompt SHALL be ≤150 tokens

#### Scenario: All prompts fit in 8k context

**Given** the system is executing any phase
**When** a prompt is built
**Then** the prompt SHALL be ≤500 tokens
**And** the system SHALL support models with 8k context windows

### Requirement: Confidence Scoring

The system SHALL assign confidence scores to all detection results based on detection method and data quality.

#### Scenario: High confidence from deterministic parser

**Given** phase 4 uses a deterministic parser
**When** the parser successfully extracts dependencies
**Then** confidence SHALL be `Confidence::High`

#### Scenario: Medium confidence from LLM with limited data

**Given** phase 6a (runtime detection) has only 5 files to analyze
**When** the LLM returns a result
**Then** the result MAY indicate `confidence: medium`

#### Scenario: Low confidence from LLM with ambiguous data

**Given** phase 6c (entrypoint detection) finds multiple potential entrypoints
**When** the LLM returns a result
**Then** the result SHOULD indicate `confidence: low`

### Requirement: Heuristic Logging

The system SHALL log all LLM inputs and outputs to enable future optimization through heuristic extraction.

#### Scenario: Logging LLM phase execution

**Given** phase 6a (runtime detection) executes
**When** the LLM returns a result
**Then** the system SHALL log:
- Repository identifier
- Phase name
- Input hash
- Output hash
- Full input JSON
- Full output JSON
- Latency in milliseconds

#### Scenario: JSONL log format

**Given** the system executes multiple detections
**When** heuristic logging is active
**Then** logs SHALL be written in JSONL format
**And** each line SHALL be valid JSON
**And** logs SHALL be appendable (no file overwrite)

#### Scenario: Future heuristic extraction (non-functional)

**Given** logs contain 1000+ executions
**When** analyzed for patterns
**Then** common patterns SHOULD be extractable as deterministic heuristics
**And** heuristics SHOULD allow skipping LLM calls when pattern matches

### Requirement: Code-Based Extraction

The system SHALL extract structured data from code and configuration files before invoking LLM prompts, reducing context size.

#### Scenario: Port extraction from code

**Given** a Node.js service with `app.listen(3000)`
**When** phase 6e (port discovery) executes
**Then** the port extractor SHALL find port 3000 via regex
**And** the LLM prompt SHALL include the extracted snippet, not full file

#### Scenario: Environment variable extraction

**Given** a service with `.env.example` containing `DATABASE_URL=`
**When** phase 6f (env vars discovery) executes
**Then** the env vars extractor SHALL find `DATABASE_URL`
**And** the LLM prompt SHALL include the extracted variable names, not full file

#### Scenario: Health check extraction from routes

**Given** a Spring Boot service with `@GetMapping("/actuator/health")`
**When** phase 6g (health check discovery) executes
**Then** the health check extractor SHALL find the route via regex
**And** the LLM prompt SHALL include the matched route definition

### Requirement: Framework Defaults

The system SHALL apply framework-specific defaults for health checks when no explicit endpoint is found.

#### Scenario: Spring Boot health defaults

**Given** a Spring Boot service with no custom health endpoints
**When** phase 6g (health check discovery) executes
**Then** the system SHALL recommend `/actuator/health/liveness` for liveness
**And** it SHALL recommend `/actuator/health/readiness` for readiness

#### Scenario: Next.js health defaults

**Given** a Next.js service with no custom health endpoints
**When** phase 6g completes
**Then** the system SHALL recommend `/api/health` for combined health check

### Requirement: Migration Compatibility

The system SHALL support gradual migration from tool-based to pipeline architecture.

#### Scenario: Dual-mode execution

**Given** the migration is in progress
**When** the `--mode both` flag is set
**Then** the system SHALL run both tool-based and pipeline approaches
**And** it SHALL compare outputs
**And** it SHALL log token usage and latency for both

#### Scenario: Pipeline opt-in

**Given** the pipeline is implemented
**When** the user sets `--pipeline` flag
**Then** the system SHALL use the pipeline architecture
**And** it SHALL NOT use the tool-based approach

#### Scenario: Legacy fallback

**Given** the pipeline is the default
**When** the user sets `--legacy-tools` flag
**Then** the system SHALL use the tool-based approach
**And** it SHALL emit a deprecation warning

### Requirement: Performance Targets

The system SHALL achieve significant performance improvements over the tool-based approach.

#### Scenario: Token usage reduction

**Given** a repository analyzed with both approaches
**When** metrics are compared
**Then** the pipeline SHALL use ≥80% fewer tokens than the tool-based approach

#### Scenario: Latency improvement

**Given** a monorepo with 3 services
**When** analyzed with both approaches
**Then** the pipeline SHALL complete in ≤50% of the tool-based approach time

#### Scenario: Accuracy parity

**Given** a test corpus of 50+ repositories
**When** both approaches analyze all repositories
**Then** the pipeline SHALL produce ≥95% identical results

#### Scenario: Small model support

**Given** the system uses Qwen 2.5 Coder 0.5B (8k context)
**When** analyzing any repository
**Then** all prompts SHALL fit within the model's context window
**And** detection SHALL complete successfully

## MODIFIED Requirements

None. This is a new capability.

## REMOVED Requirements

None. Tool-based approach will be deprecated separately after validation.
