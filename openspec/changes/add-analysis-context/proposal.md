# Change: Add Global AnalysisContext and WorkflowPhase Trait

## Why

The current pipeline orchestration passes different result types between phases (ScanResult, ClassifyResult, StructureResult, etc.) with verbose parameter lists. This creates tight coupling, makes it difficult to share state between phases, and complicates the orchestrator logic with manual progress tracking and error handling for each phase.

A unified `AnalysisContext` that accumulates state through the pipeline would simplify orchestration, reduce parameter passing, enable shared state (like progress handlers and heuristic loggers), and make the pipeline more maintainable and extensible.

## What Changes

- **Add `AnalysisContext` struct** that holds:
  - Repository path
  - All phase results (scan, classify, structure, dependencies, etc.)
  - Shared resources (LLM client, stack registry, progress handler, heuristic logger)
  - Phase timing information

- **Add `ServiceContext` struct** for service-level phases:
  - Service reference and repository-level results (scan, dependencies)
  - Shared resources (LLM client, stack registry, heuristic logger)
  - Immutable access to prevent service phases from modifying repository state

- **Add `WorkflowPhase` trait** for repository-level phases:
  - `async fn execute(&self, context: &mut AnalysisContext) -> Result<()>`
  - `fn name(&self) -> &'static str`

- **Add `ServicePhase` trait** for service-level phases:
  - `async fn execute(&self, context: &ServiceContext) -> Result<ServicePhaseResult>`
  - `fn name(&self) -> &'static str`

- **Refactor PipelineOrchestrator** to:
  - Initialize AnalysisContext once
  - Execute repository-level phases (scan, classify, structure, dependencies, build_order) using `WorkflowPhase` trait
  - Create ServiceContext for each service and execute service phases using `ServicePhase` trait
  - Handle progress reporting and error handling uniformly
  - Reduce code duplication in phase execution

- **Update repository-level phases** to:
  - Implement the `WorkflowPhase` trait
  - Read inputs from AnalysisContext
  - Write outputs to AnalysisContext

- **Update service-level phases** to:
  - Implement the `ServicePhase` trait
  - Read inputs from ServiceContext
  - Return ServicePhaseResult for aggregation

## Impact

- **Affected specs**: `ai-pipeline`
- **Affected code**:
  - `src/pipeline/orchestrator.rs` - Simplified orchestration logic with two-phase execution
  - `src/pipeline/phases/*.rs` - All phases implement WorkflowPhase or ServicePhase traits
  - `src/pipeline/context.rs` - New file defining AnalysisContext
  - `src/pipeline/service_context.rs` - New file defining ServiceContext
  - `src/pipeline/phase_trait.rs` - New file defining WorkflowPhase and ServicePhase traits

- **Breaking changes**:
  - Repository phase `execute()` signatures change from `(params...) -> Result<PhaseResult>` to `(&self, &mut AnalysisContext) -> Result<()>`
  - Service phase `execute()` signatures change from `(service, params...) -> Result<PhaseResult>` to `(&self, &ServiceContext) -> Result<ServicePhaseResult>`
  - Phase results accessed via context instead of return values

- **Benefits**:
  - Reduced parameter passing (from 3-5 params per phase to 1)
  - Clear separation of repository-level vs service-level concerns
  - Type-safe phase isolation (service phases can't modify repository state)
  - Centralized state management
  - Easier to add new phases (implement trait and register)
  - Easier to add cross-cutting concerns (caching, retry logic, etc.)
  - More testable (mock contexts instead of individual dependencies)
