# Change: Refactor Wolfi BuildKit Architecture

## Why
- Build systems use `/tmp` for caches → no persistence across builds (8 affected systems: Maven, Gradle, Poetry, Pipenv, npm, pnpm, yarn, dotnet)
- Framework-specific logic scattered in Runtime implementations and Pipeline phases → hard to maintain
- BuildKit commands merged into single layer with `&&` → poor cache utilization, no intermediate caching
- Missing separation of concerns between Runtime and Framework traits

## What Changes
- **BREAKING**: Remove `build.artifacts` field from schema (use `runtime.copy` instead as single source of truth)
- **BREAKING**: Remove `confidence` field from BuildMetadata (unused in decision-making)
- Fix cache directories for all affected build systems: `/tmp/*` → `/root/.cache/`, `/root/.m2/`, `/root/.gradle/`
- Refactor BuildKit LLB: remove 6 code duplicates, separate commands into individual layers for better caching
- Add Framework trait methods: `runtime_env_vars()`, `entrypoint_command()`, modify `health_endpoints(files)`
- Move hardcoded framework logic from `assemble.rs`, `jvm.rs`, `python.rs` to respective Framework implementations
- Add 6 PHP extensions: curl, json, session, tokenizer, fileinfo, iconv + framework-specific detection
- Add `--service` CLI flag for monorepo support with strict error handling (no fallback to first service)
- Add unique service name validation in detection (append counter for duplicates)
- Add test fixtures: `static-html` (nginx), `dockerfile-exists` (docker/dockerfile:1 frontend)
- Enforce health endpoint requirement for all container tests (mandatory HTTP servers)
- Java build handling: Gradle copies all JARs, Maven uses dependency:copy-dependencies

## Impact
- **Affected specs**: buildkit-frontend, build-systems, framework-detection, testing, schema
- **Affected code**: 27 files across BuildKit, build systems, frameworks, runtimes, pipeline, CLI
- **Breaking changes**: 2 (artifacts field removal, confidence field removal)
- **Migration**: Update all UniversalBuild JSON files to remove `artifacts` and `confidence` fields
