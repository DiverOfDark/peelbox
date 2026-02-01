<!-- OPENSPEC:START -->
# OpenSpec Instructions

These instructions are for AI assistants working in this project.

Always open `@/openspec/AGENTS.md` when the request:
- Mentions planning or proposals (words like proposal, spec, change, plan)
- Introduces new capabilities, breaking changes, architecture shifts, or big performance/security work
- Sounds ambiguous and you need the authoritative spec before coding

Use `@/openspec/AGENTS.md` to learn:
- How to create and apply change proposals
- Spec format and conventions
- Project structure and guidelines

Keep this managed block so 'openspec update' can refresh the instructions.

<!-- OPENSPEC:END -->

# PROJECT KNOWLEDGE BASE

**Generated:** 2026-01-12T21:15:00Z
**Commit:** e7ca064
**Branch:** feature/buildkit-client

## OVERVIEW
peelbox is a Rust-based AI-powered buildkit frontend for intelligent build command detection. It uses a 9-phase pipeline to produce distroless Wolfi-based container images.

## STRUCTURE
```
.
├── src/
│   ├── buildkit/    # Native gRPC client & LLB generation
│   ├── llm/         # Pluggable backends (Embedded, Ollama, Claude, etc.)
│   ├── pipeline/    # 9-phase deterministic orchestration
│   ├── stack/       # Language, BuildSystem, and Framework traits
│   └── main.rs      # CLI Entry point
└── tests/           # Integration tests with OCI/Docker verification
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add Language/Tool | `src/stack/` | Implement BuildSystem or Framework traits |
| Pipeline Logic | `src/pipeline/` | Modify WorkflowPhase or ServicePhase |
| BuildKit/LLB | `src/buildkit/` | Core gRPC and LLB graph builders |
| LLM Backends | `src/llm/` | Add new providers via LLMClient trait |

## CONVENTIONS
- **Strict Distroless**: Final images must contain ZERO apk binary or metadata.
- **Merge-First**: Use `MergeOp` with independent snapshots to minimize layers.
- **Isolated Tests**: Use unique `PEELBOX_CACHE_DIR` per test process.
- **Imports**: Standard library (`std::*`), then external, then crate-local.
- **Error Handling**: `anyhow::Result` with `context()` for apps; `thiserror` for core logic.
- **Documentation**: Minimalist. Code must be self-documenting. Remove "todo"/"debug" before commit.

## ANTI-PATTERNS
- **No unwrap/expect**: Use proper error handling in non-test code.
- **No backwards compatibility**: Breaking changes preferred over technical debt.
- **No manual buildctl**: Use native gRPC implementation in `src/buildkit/`.
- **Zero Dead Code**: Always remove unused code immediately.
- **Clean Slate**: Refactor properly rather than patching.
- **No Naive Container Debugging**: When diagnosing `container_e2e` failures, NEVER run `docker` commands blindly or guess. ALWAYS:
    1. Identify the failing fixture and its `universalbuild.json`.
    2. Manually run `peelbox build --spec <path> --context <dir> --tag <tag>` for that specific context.
    3. Analyze the actual build logs (stdout/stderr) from the command to find the root cause.

## DEVELOPMENT PRINCIPLES
- **Single Responsibility**: 
    - `BuildSystem`: Commands, packages, cache ONLY.
    - `Runtime`: Base images, ports, entrypoints ONLY.
    - `Framework`: Framework-specific defaults ONLY.
- **No Historical Comments**: Documentation reflects current state ONLY.

## LLM TESTING SAFETY
- **CUDA IS MANDATORY LOCALLY**: LLM tests **MUST NEVER** run without `--features cuda` locally.
- **RAM Safety**: Embedded LLM bails if <4GB RAM remains after loading on CPU.
- **Serial Execution**: Sensitive tests (Docker, LLM) MUST use `serial-tests` group in `nextest.toml`.
- **CI Mode**: CI must always run in `replay` mode (using recordings).


## COMMANDS
```bash
# Full local verification
cargo nextest run --release --no-default-features --features cuda

# Single test
cargo nextest run <substring>

# Accurate Coverage
cargo llvm-cov clean --workspace
cargo llvm-cov nextest --release --no-default-features
```

## NOTES
- Embedded LLM fails on CPU if <4GB RAM remains after loading (Safety check).
- CI runs in `replay` mode using recordings in `tests/recordings/`.
