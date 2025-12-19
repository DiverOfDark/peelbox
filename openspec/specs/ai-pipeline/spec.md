# ai-pipeline Specification

## Purpose
TBD - created by archiving change restructure-ai-pipeline. Update Purpose after archive.
## Requirements
### Requirement: Pipeline Orchestration

The system SHALL provide a `PipelineOrchestrator` that coordinates build system detection through a multi-phase analysis pipeline using a global `AnalysisContext` and trait-based phase execution.

#### Scenario: Successful detection
- **WHEN** `PipelineOrchestrator.execute()` is called with a valid repository path
- **THEN** the orchestrator initializes an `AnalysisContext` with the path and shared resources
- **AND** executes all phases sequentially by calling their `execute(&mut context)` methods
- **AND** returns the final `Vec<UniversalBuild>` from `context.assemble.unwrap()`

#### Scenario: Phase failure
- **WHEN** a phase returns an error from its `execute()` method
- **THEN** the orchestrator stops execution and returns the error
- **AND** emits a progress event indicating which phase failed

#### Scenario: Service analysis loop
- **WHEN** executing service-specific phases (runtime, build, entrypoint, port, env_vars, health)
- **THEN** the orchestrator iterates over services from `context.structure.services`
- **AND** creates a `ServiceContext` for each service
- **AND** executes each `ServicePhase` with the `ServiceContext`
- **AND** collects `ServicePhaseResult` instances per service
- **AND** stores aggregated results in `context.service_analyses`

---

### Requirement: Conversation Management

The system SHALL provide a `ConversationManager` that maintains LLM message history and handles communication.

#### Scenario: Message history maintained
- **WHEN** tool responses are added to the conversation
- **THEN** subsequent LLM requests include the complete message history

#### Scenario: System prompt applied
- **WHEN** a new conversation is started
- **THEN** the system prompt is set as the first message

---

### Requirement: LLM Client Abstraction

The system SHALL abstract LLM communication behind an `LLMClient` trait to enable testing and provider flexibility.

#### Scenario: GenAI provider
- **WHEN** using the GenAI-based client
- **THEN** requests are sent to the configured provider (Ollama, Claude, OpenAI, etc.)

#### Scenario: Mock client for testing
- **WHEN** using a mock client
- **THEN** predefined responses are returned without network calls

---

### Requirement: Unified Tool System

The system SHALL provide a `ToolSystem` that manages tool definitions, execution, and caching.

#### Scenario: Tool execution
- **WHEN** the LLM requests a tool call
- **THEN** `ToolSystem.execute()` runs the tool and returns the result

#### Scenario: Tool caching
- **WHEN** the same tool is called with identical arguments within a session
- **THEN** the cached result is returned without re-execution

#### Scenario: Terminal tool detection
- **WHEN** `submit_detection` is called
- **THEN** `ToolSystem.is_terminal_tool()` returns true to signal analysis completion

---

### Requirement: FileSystem Abstraction

The system SHALL abstract file operations behind a `FileSystem` trait for testability and security.

#### Scenario: Path validation
- **WHEN** a tool requests a file outside the repository root
- **THEN** the operation fails with a path traversal error

#### Scenario: Mock filesystem for testing
- **WHEN** using a mock filesystem
- **THEN** tools operate on in-memory file structures

---

### Requirement: Validation System

The system SHALL provide centralized validation for `UniversalBuild` results.

#### Scenario: Schema validation
- **WHEN** validating a `UniversalBuild`
- **THEN** all required fields are checked against the schema

#### Scenario: Business rule validation
- **WHEN** validating a `UniversalBuild`
- **THEN** business rules are applied (non-empty commands, valid image names, etc.)

#### Scenario: Validation feedback to LLM
- **WHEN** validation fails
- **THEN** detailed error messages are returned to guide LLM retry

---

### Requirement: Progress Events

The system SHALL emit progress events during analysis for observability.

#### Scenario: Tool call events
- **WHEN** a tool is called
- **THEN** `AnalysisEvent::ToolCalled` is emitted with tool name and arguments

#### Scenario: Completion events
- **WHEN** analysis completes successfully
- **THEN** `AnalysisEvent::Completed` is emitted with duration and confidence

#### Scenario: Optional progress callback
- **WHEN** no progress callback is provided
- **THEN** analysis proceeds without emitting events

---

### Requirement: Tool Definitions

The system SHALL provide the following tools for repository analysis:

#### Scenario: list_files tool
- **WHEN** `list_files` is called with a path and optional pattern
- **THEN** matching files in the directory are returned

#### Scenario: read_file tool
- **WHEN** `read_file` is called with a file path
- **THEN** the file contents are returned (up to configured line limit)

#### Scenario: search_files tool
- **WHEN** `search_files` is called with a glob pattern
- **THEN** matching file paths across the repository are returned

#### Scenario: get_file_tree tool
- **WHEN** `get_file_tree` is called
- **THEN** a JSON tree structure of the repository is returned

#### Scenario: grep_content tool
- **WHEN** `grep_content` is called with a regex pattern
- **THEN** matching lines with file paths and line numbers are returned

#### Scenario: get_best_practices tool
- **WHEN** `get_best_practices` is called with language and build system
- **THEN** a recommended build template is returned

#### Scenario: submit_detection tool
- **WHEN** `submit_detection` is called with a UniversalBuild
- **THEN** the result is validated and returned as the analysis output

### Requirement: Global Analysis Context

The system SHALL provide an `AnalysisContext` struct that accumulates state throughout the multi-phase pipeline.

#### Scenario: Context initialization
- **WHEN** the pipeline orchestrator starts analysis
- **THEN** an `AnalysisContext` is created with repository path, LLM client, and shared resources
- **AND** all phase result fields are initialized to `None`

#### Scenario: Phase result storage
- **WHEN** a pipeline phase completes successfully
- **THEN** the phase writes its result to the corresponding `Option<PhaseResult>` field in the context
- **AND** subsequent phases can access the result via context

#### Scenario: Shared resource access
- **WHEN** a phase needs to log a heuristic
- **THEN** the phase accesses `context.heuristic_logger` without requiring it as a parameter
- **AND** the same logger instance is used across all phases

#### Scenario: Missing prerequisite detection
- **WHEN** a phase attempts to read a prerequisite result that is `None`
- **THEN** the phase panics with a clear error message indicating the missing prerequisite
- **AND** this is considered a programmer error caught in tests

---

### Requirement: Workflow Phase Trait

The system SHALL define a `WorkflowPhase` trait for uniform repository-level phase execution interface.

#### Scenario: Repository phase execution
- **WHEN** a repository-level phase implementing `WorkflowPhase` is executed
- **THEN** the phase's `execute` method receives a mutable reference to `AnalysisContext`
- **AND** the phase reads inputs from context and writes outputs to context
- **AND** the method returns `Result<()>` to signal success or failure

#### Scenario: Phase naming
- **WHEN** querying a phase for its name
- **THEN** the phase's `name()` method returns a static string (e.g., "scan", "classify")
- **AND** this name is used for progress reporting and error messages

#### Scenario: Async phase execution
- **WHEN** an async phase is executed (e.g., classify, structure, dependencies)
- **THEN** the `execute` method is async and can await LLM calls or I/O operations

#### Scenario: Sync phase execution
- **WHEN** a deterministic phase is executed (e.g., build_order, cache)
- **THEN** the `execute` method can be async but performs only synchronous operations

---

### Requirement: Service Phase Trait

The system SHALL define a `ServicePhase` trait for service-specific phase execution with dedicated `ServiceContext`.

#### Scenario: Service phase execution
- **WHEN** a service-level phase implementing `ServicePhase` is executed
- **THEN** the phase's `execute` method receives a reference to `ServiceContext`
- **AND** the phase analyzes the specific service from the context
- **AND** the method returns `Result<ServicePhaseResult>` with the analysis result

#### Scenario: ServiceContext creation
- **WHEN** the orchestrator analyzes a service
- **THEN** it creates a `ServiceContext` with service reference and shared resources
- **AND** passes repository-level results (scan, dependencies) as immutable references
- **AND** provides access to LLM client, stack registry, and heuristic logger

#### Scenario: Service phase isolation
- **WHEN** multiple services are analyzed
- **THEN** each service gets its own `ServiceContext` instance
- **AND** service phases cannot modify repository-level state
- **AND** service phase results are collected independently

---

### Requirement: Simplified Pipeline Orchestration

The system SHALL simplify the orchestrator by using trait-based phase execution.

#### Scenario: Generic phase execution loop
- **WHEN** the orchestrator executes the pipeline
- **THEN** it iterates over a list of `Box<dyn WorkflowPhase>` instances
- **AND** calls each phase's `execute(&mut context)` method in sequence
- **AND** handles progress reporting and error logging uniformly

#### Scenario: Reduced parameter passing
- **WHEN** executing a phase
- **THEN** the orchestrator passes only the `AnalysisContext` (not individual results and resources)
- **AND** phase signatures are simplified from 3-5 parameters to 1

#### Scenario: Phase timing metadata
- **WHEN** a phase completes
- **THEN** the orchestrator records the phase duration in the context
- **AND** emits a `PhaseComplete` progress event with the duration

---

