# AI Analysis Pipeline

The AI analysis pipeline orchestrates LLM-based build system detection through iterative tool calling.

## ADDED Requirements

### Requirement: Pipeline Orchestration

The system SHALL provide an `AnalysisPipeline` that coordinates build system detection through iterative LLM conversation with tool calls.

#### Scenario: Successful detection
- **WHEN** `AnalysisPipeline.analyze()` is called with a valid repository path
- **THEN** the pipeline iterates through LLM tool calls until `submit_detection` is called
- **AND** returns a validated `UniversalBuild` result

#### Scenario: Maximum iterations exceeded
- **WHEN** the LLM does not call `submit_detection` within the configured maximum iterations
- **THEN** the pipeline returns an `AnalysisError::MaxIterationsExceeded` error

#### Scenario: Timeout exceeded
- **WHEN** the total analysis time exceeds the configured timeout
- **THEN** the pipeline returns an `AnalysisError::Timeout` error

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
