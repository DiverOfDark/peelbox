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

## 1. Comment and Documentation Policy
- **NO UNNECESSARY COMMENTS**: Code must be self-documenting.
- **MANDATORY COMMENT REMOVAL**: Remove all "todo", "debug", or temporary comments before committing.
- **EXCEPTIONS**: Only for truly complex logic, security, or mandatory BDD comments.

## 2. LLM Testing Safety
- **CUDA IS MANDATORY**: LLM tests **MUST NEVER** run without `--features cuda` locally. Parallel CPU inference will crash the host.
- **SERIAL GROUPS**: Sensitive tests (Docker, LLM) must use `serial-tests` group in `nextest.toml`.
- **ISOLATED CACHE**: Use unique `PEELBOX_CACHE_DIR` per test process.

## 3. Tooling
- Use `cargo-nextest` for execution.
- Use `cargo-llvm-cov` for coverage (always `clean` first).