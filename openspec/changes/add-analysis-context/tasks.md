# Implementation Tasks

## 1. Core Infrastructure
- [ ] 1.1 Create `src/pipeline/context.rs` with `AnalysisContext` struct
- [ ] 1.2 Create `src/pipeline/service_context.rs` with `ServiceContext` struct
- [ ] 1.3 Create `src/pipeline/phase_trait.rs` with `WorkflowPhase` and `ServicePhase` traits
- [ ] 1.4 Add context, service_context, and phase_trait modules to `src/pipeline/mod.rs`
- [ ] 1.5 Add phase result fields to AnalysisContext for repository-level phases
- [ ] 1.6 Define ServicePhaseResult enum for service-level phase outputs

## 2. Phase Refactoring (Deterministic)
- [ ] 2.1 Refactor `scan` phase to implement WorkflowPhase
- [ ] 2.2 Refactor `build_order` phase to implement WorkflowPhase
- [ ] 2.3 Refactor `cache` phase to implement WorkflowPhase
- [ ] 2.4 Refactor `root_cache` phase to implement WorkflowPhase
- [ ] 2.5 Refactor `assemble` phase to implement WorkflowPhase

## 3. Phase Refactoring (Repository-Level LLM-based)
- [ ] 3.1 Refactor `classify` phase to implement WorkflowPhase
- [ ] 3.2 Refactor `structure` phase to implement WorkflowPhase
- [ ] 3.3 Refactor `dependencies` phase to implement WorkflowPhase

## 4. Phase Refactoring (Service-Level)
- [ ] 4.1 Refactor `runtime` phase to implement ServicePhase
- [ ] 4.2 Refactor `build` phase to implement ServicePhase
- [ ] 4.3 Refactor `entrypoint` phase to implement ServicePhase
- [ ] 4.4 Refactor `native_deps` phase to implement ServicePhase
- [ ] 4.5 Refactor `port` phase to implement ServicePhase
- [ ] 4.6 Refactor `env_vars` phase to implement ServicePhase
- [ ] 4.7 Refactor `health` phase to implement ServicePhase

## 5. Orchestrator Refactoring
- [ ] 5.1 Update PipelineOrchestrator to initialize AnalysisContext
- [ ] 5.2 Create repository-level phase instances (scan, classify, structure, dependencies, build_order)
- [ ] 5.3 Create service-level phase instances (runtime, build, entrypoint, native_deps, port, env_vars, health)
- [ ] 5.4 Implement generic repository phase execution loop with progress tracking
- [ ] 5.5 Implement service analysis loop using ServiceContext and ServicePhase trait
- [ ] 5.6 Update error handling to use context state
- [ ] 5.7 Remove old verbose parameter passing code

## 6. Testing & Validation
- [ ] 6.1 Update unit tests for refactored repository-level phases
- [ ] 6.2 Update unit tests for refactored service-level phases
- [ ] 6.3 Update integration tests to use new orchestrator
- [ ] 6.4 Verify all existing fixtures still pass
- [ ] 6.5 Add test for AnalysisContext state accumulation
- [ ] 6.6 Add test for ServiceContext creation and lifecycle
- [ ] 6.7 Run full test suite with recordings

## 7. Documentation
- [ ] 7.1 Update CLAUDE.md with AnalysisContext and ServiceContext patterns
- [ ] 7.2 Add code comments to WorkflowPhase and ServicePhase traits
- [ ] 7.3 Document context lifecycle in orchestrator
- [ ] 7.4 Document ServiceContext usage for service phases
- [ ] 7.5 Update architecture diagram if exists
