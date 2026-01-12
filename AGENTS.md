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

# Project Guidelines

## 1. Development Principles (CRITICAL)
- **NO BACKWARDS COMPATIBILITY**: Breaking changes are preferred if they improve the codebase. Never maintain compatibility with old APIs, configurations, or interfaces.
- **ZERO DEAD CODE**: Always remove dead code immediately. If you find unused code, delete it.
- **CLEAN SLATE**: When refactoring, completely remove old code and update all references. The codebase should read as if it was always implemented the current way.
- **SINGLE RESPONSIBILITY**: Each module must have a single, well-defined responsibility.
    - **BuildSystem Trait**: Build-time concerns ONLY (packages, commands, cache).
    - **Runtime Trait**: Runtime concerns ONLY (base images, entrypoints, health checks).
    - **Framework Trait**: Framework-specific patterns ONLY (default ports, special endpoints).
    - **Pipeline Phases**: Discrete logic per phase (Scan, Classify, Structure, etc.).

## 2. Comment and Documentation Policy
- **NO UNNECESSARY COMMENTS**: Code must be self-documenting. Use descriptive names instead of explanatory comments.
- **MANDATORY COMMENT REMOVAL**: Remove all "todo", "debug", or temporary comments before committing.
- **NO HISTORICAL COMMENTS**: Never include comments explaining past changes (e.g., "removed X because...").
- **EXCEPTIONS**: Only for truly complex logic, security, or mandatory BDD comments.

## 3. LLM Testing and Safety
- **CUDA IS MANDATORY LOCALLY**: LLM tests **MUST NEVER** run without `--features cuda` locally. Parallel CPU inference will crash the host.
- **SERIAL GROUPS**: Sensitive tests (Docker, LLM) must use `serial-tests` group in `nextest.toml`.
- **ISOLATED CACHE**: Use unique `PEELBOX_CACHE_DIR` per test process to avoid race conditions.
- **RAM SAFETY**: Embedded LLM will fail to start on CPU if less than 4GB of RAM remains after loading.
- **REPLAY MODE IN CI**: CI must always run in `replay` mode (no real LLM calls).

## 4. Architecture: Wolfi-First & Distroless
- **WOLFI EXCLUSIVE**: Use Wolfi packages exclusively for all images.
- **DISTROLESS BY DEFAULT**: Final images must be truly distroless (no `apk` binary or metadata).
- **OPTIMIZED LLB**: Use `MergeOp` with independent snapshots to minimize layers and avoid whiteouts.
- **CONTEXT OPTIMIZATION**: Always use `.gitignore` filtering to minimize context transfer to BuildKit.

## 5. Tooling
- **NEXTEST**: Use `cargo-nextest` for all test executions.
- **LLVM-COV**: Use `cargo-llvm-cov` for coverage. Always `clean` first.
- **CLIPPY & FMT**: Code must be clean and formatted according to project standards.
