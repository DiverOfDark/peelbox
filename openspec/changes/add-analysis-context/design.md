# Design: Global AnalysisContext and WorkflowPhase Trait

## Context

The current pipeline architecture uses explicit result passing between phases, where each phase function returns a typed result (ScanResult, ClassifyResult, etc.) that's then passed to dependent phases. The orchestrator manually manages progress tracking, error handling, and resource passing for each of the 15+ phases.

This works but has limitations:
- Verbose parameter lists (3-5 parameters per phase)
- Tight coupling between phases (dependency changes require signature updates)
- Difficult to add cross-cutting concerns (retry logic, caching, validation)
- Hard to share state (progress handlers, heuristic loggers passed repeatedly)
- Orchestrator has ~350 lines of repetitive boilerplate

## Goals / Non-Goals

**Goals:**
- Simplify phase signatures to single context parameter
- Enable shared state across phases without parameter passing
- Make adding new phases easier (implement trait, register in orchestrator)
- Reduce orchestrator boilerplate through generic phase execution
- Maintain type safety and clear data flow
- Keep backward compatibility with existing phase logic

**Non-Goals:**
- Change the 9-phase pipeline structure
- Modify phase execution order or semantics
- Add new phases or capabilities
- Change external API (DetectionService remains unchanged)
- Introduce async complexity beyond what already exists

## Decisions

### Decision 1: AnalysisContext as Mutable State Container

**What:** Create a single `AnalysisContext` struct that holds:
- Immutable inputs: `repo_path`, `llm_client`, `stack_registry`
- Mutable phase results: `scan`, `classify`, `structure`, etc. as `Option<T>`
- Shared resources: `progress_handler`, `heuristic_logger`
- Metadata: phase timings, current phase name

**Why:**
- Single source of truth for pipeline state
- Clear ownership (orchestrator owns context, phases borrow mutably)
- Type-safe access with Option unwrapping (panic = programmer error)
- Easy to add new fields without changing phase signatures

**Alternatives considered:**
- **Builder pattern**: More ceremony, no clear benefit for sequential pipeline
- **Separate context per phase**: Loses cross-phase state sharing benefit
- **Arc<RwLock<Context>>**: Unnecessary complexity, single-threaded execution

### Decision 2: WorkflowPhase Trait for Uniform Interface

**What:**
```rust
#[async_trait]
pub trait WorkflowPhase: Send + Sync {
    async fn execute(&self, context: &mut AnalysisContext) -> Result<()>;
    fn name(&self) -> &'static str;
}
```

**Why:**
- Enables generic orchestration logic (one loop instead of 15+ phase calls)
- Phases are self-contained (read from context, write to context)
- Easy to add hooks (before/after phase execution)
- Testable in isolation (mock context)

**Alternatives considered:**
- **Free functions**: Keep current design, miss opportunity for trait-based dispatch
- **Separate sync/async traits**: Complicates orchestrator, minor performance gain not worth it
- **Phase enum**: Less extensible, requires modifying enum for new phases

### Decision 3: Phase Result Storage in Context

**What:** Store phase results as `Option<PhaseResult>` fields in AnalysisContext:
```rust
pub struct AnalysisContext {
    // ... immutable fields ...
    pub scan: Option<ScanResult>,
    pub classify: Option<ClassifyResult>,
    pub structure: Option<StructureResult>,
    // ... etc ...
}
```

**Why:**
- Type-safe access (can't access result before phase runs)
- Clear dependency tracking (phase panics if prerequisite is None)
- Explicit lifecycle (Option::Some signals phase completion)

**Alternatives considered:**
- **HashMap<String, Any>**: Type-unsafe, runtime errors instead of compile errors
- **Separate context builders**: Over-engineered for linear pipeline
- **Result as trait object**: Boxing overhead, loses type information

### Decision 4: Separate ServiceContext for Service-Level Phases

**What:** Create a dedicated `ServiceContext` struct for service-specific phases (runtime, build, entrypoint, native_deps, port, env_vars, health):
```rust
pub struct ServiceContext<'a> {
    pub service: &'a Service,
    pub repo_path: &'a Path,
    pub scan: &'a ScanResult,
    pub dependencies: &'a DependencyResult,
    pub llm_client: &'a dyn LLMClient,
    pub stack_registry: &'a StackRegistry,
    pub heuristic_logger: &'a HeuristicLogger,
}

#[async_trait]
pub trait ServicePhase: Send + Sync {
    async fn execute(&self, context: &ServiceContext) -> Result<ServicePhaseResult>;
    fn name(&self) -> &'static str;
}
```

**Why:**
- Clear separation: repository-level phases vs service-level phases
- Type safety: Service phases can't accidentally access wrong context
- Better testability: Mock ServiceContext independently
- Cleaner orchestrator: Service loop has distinct phase type

**Alternatives considered:**
- **Option A (Shared Context with Vec)**: Mixes repository and service concerns, harder to reason about
- **Service as context field**: Still requires conditional logic, less type-safe

### Decision 5: Orchestrator Simplification

**What:** Refactor orchestrator to use phase registries and generic execution:
```rust
// Repository-level phases
let repo_phases: Vec<Box<dyn WorkflowPhase>> = vec![
    Box::new(ScanPhase),
    Box::new(ClassifyPhase),
    Box::new(StructurePhase),
    Box::new(DependenciesPhase),
    Box::new(BuildOrderPhase),
];

for phase in repo_phases {
    self.execute_phase(phase, &mut context).await?;
}

// Service-level phases
let service_phases: Vec<Box<dyn ServicePhase>> = vec![
    Box::new(RuntimePhase),
    Box::new(BuildPhase),
    Box::new(EntrypointPhase),
    Box::new(NativeDepsPhase),
    Box::new(PortPhase),
    Box::new(EnvVarsPhase),
    Box::new(HealthPhase),
];

for service in &context.structure.services {
    let service_ctx = ServiceContext::new(service, &context);
    for phase in &service_phases {
        phase.execute(&service_ctx).await?;
    }
}
```

**Why:**
- Reduces orchestrator from ~350 lines to ~100 lines
- Uniform progress tracking and error handling
- Clear distinction between repository and service phases
- Easy to add conditional phase execution or retry logic

**Alternatives considered:**
- **Keep current structure**: Miss opportunity to reduce boilerplate
- **Macro-based generation**: Too clever, harder to debug
- **Declarative pipeline DSL**: Over-engineered for current needs

## Risks / Trade-offs

### Risk 1: Breaking Change to Phase Signatures
**Impact:** All phases need refactoring, downstream code needs updates

**Mitigation:**
- Implement incrementally (one phase at a time)
- Keep old functions private until migration complete
- External API (DetectionService) unchanged, so no user impact

### Risk 2: Loss of Type Safety from Result Returns
**Trade-off:** Before: `let scan = execute_scan()? ` After: `context.scan.as_ref().unwrap()`

**Mitigation:**
- Document phase dependencies clearly
- Add debug assertions for phase ordering
- Panic = programmer error (caught in tests), not runtime error

### Risk 3: Context Becomes God Object
**Trade-off:** Centralized state can accumulate unrelated fields

**Mitigation:**
- Keep context focused on pipeline data only
- Use composition (context.progress.report() not context.report())
- Regular reviews to prevent scope creep

## Migration Plan

### Phase 1: Add Infrastructure (No Breaking Changes)
1. Create `context.rs` and `phase_trait.rs`
2. Keep existing phase functions, add trait implementations alongside
3. Verify compilation

### Phase 2: Refactor Orchestrator
1. Update orchestrator to use context + traits
2. Keep calling old phase functions internally
3. Verify all tests pass

### Phase 3: Remove Old Functions
1. Delete old phase function signatures
2. Clean up unused imports
3. Final test pass

### Rollback Plan
- Each phase is independent (can roll back individual phases)
- Orchestrator rollback = revert to parameter passing
- No database/external state involved, safe to rollback

## Open Questions

1. **Should context be Clone?**
   - Probably not needed (single-threaded pipeline)
   - Can add later if needed for parallel service analysis

2. **Should phases be stateful or stateless?**
   - Start with stateless (zero-sized types)
   - Can add state later if needed (e.g., caching between services)

3. **How to handle service-level phases (port, env_vars, health)?**
   - Option A: Context has `Vec<ServiceAnalysisResult>`, phases append to it
   - Option B: Separate ServiceContext passed to service phases
   - **Decision:** Option B - separate ServiceContext for better separation of concerns
