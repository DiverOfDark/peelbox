# Refactor Analysis Architecture

## Why

The current analysis pipeline has architectural misalignments between the problem domain (detecting how to build and run applications) and the implementation (16 phases with unclear boundaries and mixed concerns). Knowledge is scattered across phases instead of being organized by domain (Language, BuildSystem, Framework, Runtime, Orchestrator), LLM fallback logic is duplicated, and there's no abstraction for platform runtimes (JVM, Node, Python, etc.).

## What Changes

Restructure the analysis pipeline around **knowledge domains** with trait-based abstractions and deterministic-first, LLM-fallback strategy.

**Delivered incrementally via 14 PRs** (not a big-bang rewrite).

### Knowledge Domain Traits

Introduce four core traits representing distinct knowledge domains:

1. **`Runtime`** - Platform runtime (JVM, Node, Python, Ruby, PHP, Native)
   - Provides: base images (build + runtime), system packages, start commands, env var conventions
   - Extracts: runtime configuration (port, env vars, health) with framework hints

2. **`Framework`** - Application framework (Spring Boot, Next.js, Django, Rails, etc.)
   - Provides: build steps, default port, health endpoints, entrypoint hints
   - Extracts: framework-specific configuration
   - Used by Runtime for intelligent defaults

3. **`BuildSystem`** - Build tool (Cargo, npm, Maven, Gradle, etc.)
   - Provides: build base image, build commands, artifact paths, cache paths
   - Determines: target runtime (Cargo → Native, npm → Node)

4. **`Orchestrator`** - Monorepo manager (Turbo, Nx, Lerna, Cargo workspace)
   - Provides: workspace structure, build order, workspace-aware build commands
   - Handles: multi-package repositories

### Fallback Strategy

Each domain has deterministic implementations (existing StackRegistry pattern) plus LLM fallback:

```
TurboOrchestrator, NxOrchestrator, ... → LLMOrchestrator
CargoBuildSystem, NpmBuildSystem, ... → LLMBuildSystem
SpringBootFramework, NextJsFramework, ... → LLMFramework
JvmRuntime, NodeRuntime, ... → LLMRuntime
```

Registry tries deterministic implementations first, falls back to LLM when needed.

### Simplified Pipeline

**Current:** 16 phases (8 workflow + 8 service sub-phases)
**Proposed:** 6 phases

```
1. SCAN - Find manifests and build file tree
2. WORKSPACE STRUCTURE - Detect orchestrator, classify applications vs libraries
3. STACK IDENTIFICATION - Detect build system, runtime, framework (per application)
4. BUILD RECIPE - Extract build commands, artifacts, cache paths
5. RUNTIME CONFIGURATION - Extract runtime config (entrypoint, port, env vars, health)
6. ASSEMBLE - Combine into UniversalBuild
```

## Benefits

1. **Cleaner separation of concerns**: Each trait knows its domain
2. **Reduced LLM calls**: Framework provides defaults, Runtime uses hints (no redundant detection)
3. **Better Docker support**: Build system provides build base image, Runtime provides runtime base image (multi-stage builds)
4. **Extensibility**: Add new runtimes/frameworks/orchestrators by implementing traits
5. **Testability**: Mock individual knowledge domains independently
6. **Accuracy**: Smaller, focused LLM prompts when fallback is needed

## Impact

- **Breaking change**: Complete pipeline refactoring
- **Phases affected**: All 16 current phases replaced with 6 new phases
- **Traits introduced**: 4 new core traits (Runtime, Framework, BuildSystem, Orchestrator)
- **Code changes**: ~2000-3000 LOC refactored
- **Tests**: All e2e tests need updates (expected outputs unchanged)

## Migration Strategy

**14 incremental PRs** divided into two tracks:

**Track 1: Service Phases (PRs 1-8)** - Runtime trait + config extraction
- 8 service phases → 4 service phases

**Track 2: Workflow Phases (PRs 9-14)** - Orchestrator + workspace structure
- 8 workflow phases → 5 workflow phases

Each PR delivers independent value and passes all tests.

## Success Criteria

- All existing e2e tests pass at each PR
- No regression in detection accuracy
- Reduced average LLM token usage (5 phases → 1 phase for runtime config)
- Cleaner codebase (16 phases → 9 phases, better separation)
- Framework defaults improve accuracy (Spring Boot → 8080, Next.js → 3000)

## Dependencies on Other OpenSpec Changes

This change is **independent** and can be completed in any order relative to other pending changes.
