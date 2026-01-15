# PIPELINE KNOWLEDGE BASE

## OVERVIEW
The core 9-phase deterministic pipeline that orchestrates the detection and build generation workflow.

## STRUCTURE
```
src/pipeline/
├── phases/         # Atomic logic for each phase
├── context.rs      # AnalysisContext state container
├── phase_trait.rs  # WorkflowPhase & ServicePhase traits
└── orchestrator.rs # The Phase runner
```

## WHERE TO LOOK
| Phase Type | Location | Notes |
|------------|----------|-------|
| Repository-wide | `src/pipeline/phases/[scan, classify, structure, dependencies, build_order, root_cache]` | Logic applied to whole repo |
| Service-specific | `src/pipeline/phases/[runtime, build, entrypoint, native_deps, port, env_vars, health]` | Applied per application |
| State | `src/pipeline/context.rs` | The central source of truth |

## CONVENTIONS
- **State Passing**: All data must be shared via `AnalysisContext`.
- **Prerequisite Check**: Phases must validate that required earlier results exist in context.
- **ServiceContext**: Use the specialized `ServiceContext` for per-service phases.
- **Heuristic Logging**: Log all LLM decisions via `heuristic_logger`.

## ANTI-PATTERNS
- **Phase Coupling**: Phases should not directly call other phases.
- **Manual Execution**: Never run phases outside of the `PipelineOrchestrator`.
- **Shadow State**: Don't maintain private state across phase boundaries.
