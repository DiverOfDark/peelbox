# Refactor Analysis Architecture

## Problem Statement

The current analysis pipeline has architectural misalignments between the problem domain (detecting how to build and run applications) and the implementation (16 phases with unclear boundaries and mixed concerns).

**Key Issues:**

1. **Knowledge is scattered**: Language, build system, framework, and runtime detection are spread across multiple phases instead of being cohesive knowledge domains
2. **Phase granularity mismatch**: Port, env vars, health checks, and native deps are separate phases when they're actually properties of runtime configuration
3. **No fallback abstraction**: LLM fallback logic is duplicated in every phase instead of being a first-class concern in the knowledge domain
4. **Missing runtime abstraction**: No trait/interface representing platform runtime (JVM, Node, Python, Native) and their conventions (base images, system packages, start commands)
5. **Orchestrator capabilities underutilized**: Monorepo orchestrators (Turbo, Nx) can provide build commands and order, but this knowledge is hardcoded in phases

## Proposed Solution

Restructure the analysis pipeline around **knowledge domains** with trait-based abstractions and deterministic-first, LLM-fallback strategy.

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

1. Implement new traits alongside existing code
2. Implement new pipeline phases
3. Update DetectionService to use new pipeline
4. Migrate tests
5. Remove old phases

## Success Criteria

- All existing e2e tests pass with new pipeline
- No regression in detection accuracy
- Reduced average LLM token usage (measured in tests)
- Cleaner codebase (fewer LOC, better separation)
