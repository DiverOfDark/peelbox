## MODIFIED Requirements

### Requirement: Pipeline Orchestration

The system SHALL provide a `PipelineOrchestrator` that coordinates build system detection through a simplified 5-phase workflow pipeline using a global `AnalysisContext` and trait-based phase execution.

#### Scenario: Successful detection with reduced phases
- **WHEN** `PipelineOrchestrator.execute()` is called with a valid repository path
- **THEN** the orchestrator initializes an `AnalysisContext` with the path and shared resources
- **AND** executes exactly 5 workflow phases in sequence: ScanPhase, WorkspaceStructurePhase, RootCachePhase, ServiceAnalysisPhase, AssemblePhase
- **AND** returns the final `Vec<UniversalBuild>` from `context.assemble.unwrap()`

#### Scenario: Workspace structure detection
- **WHEN** WorkspaceStructurePhase executes
- **THEN** the phase detects monorepo orchestrator (if present) via orchestrator registry
- **AND** calls `orchestrator.workspace_structure()` to parse workspace configuration
- **AND** stores complete workspace with applications, libraries, build order, and dependency graph in context
- **AND** single-project repositories create simple workspace structure from scan

#### Scenario: Service analysis with 4 phases
- **WHEN** executing service-specific phases
- **THEN** the orchestrator iterates over services from `context.workspace.applications`
- **AND** creates a `ServiceContext` for each service
- **AND** executes exactly 4 service phases: StackIdentificationPhase, BuildPhase, RuntimeConfigPhase, CachePhase
- **AND** collects `ServicePhaseResult` instances per service
- **AND** stores aggregated results in `context.service_analyses`

#### Scenario: Phase failure
- **WHEN** a phase returns an error from its `execute()` method
- **THEN** the orchestrator stops execution and returns the error
- **AND** emits a progress event indicating which phase failed

### Requirement: Global Analysis Context

The system SHALL provide an `AnalysisContext` struct that accumulates state throughout the simplified multi-phase pipeline.

#### Scenario: Context initialization
- **WHEN** the pipeline orchestrator starts analysis
- **THEN** an `AnalysisContext` is created with repository path, LLM client, StackRegistry, and shared resources
- **AND** workflow phase result fields are initialized to `None`: `scan`, `workspace`, `root_cache`, `service_analyses`, `assemble`

#### Scenario: Workspace structure storage
- **WHEN** WorkspaceStructurePhase completes successfully
- **THEN** the phase writes `WorkspaceStructure` to `context.workspace`
- **AND** subsequent phases access workspace information (applications, libraries, build order, dependencies) via context
- **AND** no separate `classify`, `structure`, `dependencies`, or `build_order` fields exist

#### Scenario: Shared resource access
- **WHEN** a phase needs to access StackRegistry
- **THEN** the phase accesses `context.stack_registry` without requiring it as a parameter
- **AND** the same registry instance is used across all phases for stack detection

#### Scenario: Missing prerequisite detection
- **WHEN** a phase attempts to read a prerequisite result that is `None`
- **THEN** the phase panics with a clear error message indicating the missing prerequisite
- **AND** this is considered a programmer error caught in tests

---

## ADDED Requirements

### Requirement: Runtime Configuration Phase

The system SHALL provide a `RuntimeConfigPhase` that extracts all runtime properties (entrypoint, port, env vars, health, native deps) in a single pass using the Runtime trait.

#### Scenario: Unified runtime configuration extraction
- **WHEN** RuntimeConfigPhase executes for a service
- **THEN** the phase retrieves pre-detected runtime from `service_context.stack.runtime`
- **AND** the phase retrieves pre-detected framework from `service_context.stack.framework`
- **AND** calls `runtime.try_deterministic_config(files, framework)` first
- **AND** falls back to `runtime.extract_config_llm(files, framework)` if deterministic fails
- **AND** returns `RuntimeConfig` with all properties: entrypoint, port, env_vars, health, native_deps
- **AND** does NOT make separate LLM calls for each property

#### Scenario: Framework defaults utilized
- **WHEN** RuntimeConfigPhase cannot extract port from code
- **THEN** the phase uses `framework.default_ports()` as fallback
- **AND** Spring Boot services default to port 8080
- **AND** Next.js services default to port 3000
- **AND** generic services default to port 8080 if no framework detected

#### Scenario: Health endpoint from framework
- **WHEN** RuntimeConfigPhase cannot find health check in code
- **THEN** the phase uses `framework.health_endpoints()` as fallback
- **AND** Spring Boot services detect `/actuator/health`
- **AND** Rails services detect `/up`
- **AND** services without framework conventions return None

### Requirement: Stack Identification Phase

The system SHALL provide a `StackIdentificationPhase` that detects the complete technology stack (Language, BuildSystem, Framework, Runtime, Version) together as a cohesive unit.

#### Scenario: Complete stack detection
- **WHEN** StackIdentificationPhase executes for a service
- **THEN** the phase detects language via `LanguageDefinition.detect(manifest)`
- **AND** detects version via `language.detect_version(manifest_content)`
- **AND** detects build system via `BuildSystem.detect(manifest)`
- **AND** detects framework from dependencies via `StackRegistry.detect_framework()`
- **AND** maps language to runtime via `get_runtime_for_language(language)`
- **AND** stores complete `Stack { language, build_system, framework, runtime, version }` in ServiceContext
- **AND** all subsequent service phases use pre-detected stack

#### Scenario: Version extraction for runtime packages
- **WHEN** StackIdentificationPhase detects a Node.js project
- **THEN** the phase extracts version from `package.json` engines field
- **AND** stores version as `Stack.version` (e.g., "20.x")
- **AND** AssemblePhase uses version to select base image (e.g., `node:20-alpine`)

#### Scenario: No separate runtime detection phase
- **WHEN** service analysis begins
- **THEN** there is NO separate RuntimePhase, EntrypointPhase, PortPhase, EnvVarsPhase, HealthPhase, or NativeDepsPhase
- **AND** StackIdentificationPhase replaces RuntimePhase
- **AND** RuntimeConfigPhase replaces the 5 individual config phases

### Requirement: Workspace Structure Phase

The system SHALL provide a `WorkspaceStructurePhase` that replaces ClassifyPhase and StructurePhase by using MonorepoOrchestrator to parse workspace configuration.

#### Scenario: Monorepo workspace detection
- **WHEN** WorkspaceStructurePhase detects Turborepo orchestrator
- **THEN** the phase calls `TurborepoOrchestrator.workspace_structure(repo_path)`
- **AND** receives `WorkspaceStructure` with applications, libraries, build order, and dependency graph
- **AND** stores workspace in context for subsequent phases
- **AND** does NOT make separate LLM calls for classification or structure

#### Scenario: Single-project workspace
- **WHEN** WorkspaceStructurePhase detects no orchestrator
- **THEN** the phase creates single-application workspace from ScanResult
- **AND** workspace contains one application with root manifest
- **AND** workspace build_order contains single path
- **AND** workspace dependency_graph is empty

#### Scenario: Build order from orchestrator
- **WHEN** WorkspaceStructurePhase completes for monorepo
- **THEN** the phase has already populated `workspace.build_order` via orchestrator
- **AND** there is NO separate BuildOrderPhase or DependenciesPhase
- **AND** ServiceAnalysisPhase uses `workspace.build_order` directly

### Requirement: Multi-Stage Docker Support

The system SHALL generate separate base images for build and runtime stages using BuildSystem and Runtime traits.

#### Scenario: Build stage base image
- **WHEN** AssemblePhase constructs BuildStage
- **THEN** the phase calls `build_system.build_template().build_image`
- **AND** uses build-specific base image (e.g., `maven:3.9-jdk-21`, `node:20-alpine`, `rust:1.75-alpine`)
- **AND** does NOT use runtime base image for build stage

#### Scenario: Runtime stage base image
- **WHEN** AssemblePhase constructs RuntimeStage
- **THEN** the phase calls `runtime.runtime_base_image(version)`
- **AND** uses runtime-specific base image (e.g., `eclipse-temurin:21-jre-alpine`, `node:20-alpine`, `alpine:latest`)
- **AND** version parameter comes from `Stack.version`

#### Scenario: Health endpoint in runtime stage
- **WHEN** AssemblePhase constructs RuntimeStage
- **THEN** the phase populates `RuntimeStage.health` from `RuntimeConfig.health`
- **AND** health check includes endpoint, interval, timeout, and retries
- **AND** services without health checks have `health: None`
