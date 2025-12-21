# Design Document: Analysis Architecture Refactoring

## Overview

Refactor the analysis pipeline around knowledge domain traits (Runtime, Framework, BuildSystem, Orchestrator) while delivering value incrementally through 14 independent PRs.

## Architecture Principles

1. **Knowledge domains as traits**: Each domain (Runtime, Framework, BuildSystem, Orchestrator) is a trait with deterministic + LLM implementations
2. **Deterministic-first, LLM-fallback**: Registry tries deterministic implementations before LLM
3. **Incremental delivery**: 14 small PRs, each delivers standalone value, all tests pass at each step
4. **Version-aware**: Runtime methods accept version parameter for flexible package selection

## Core Traits

### Runtime Trait

**Responsibility**: Platform execution environment and runtime configuration extraction

```rust
pub trait Runtime: Send + Sync {
    fn name(&self) -> &str;

    // Configuration extraction (deterministic + LLM pattern)
    fn try_deterministic_config(
        &self,
        files: &FileTree,
        framework: Option<&dyn Framework>
    ) -> Option<RuntimeConfig>;

    async fn extract_config_llm(
        &self,
        files: &FileTree,
        framework: Option<&dyn Framework>
    ) -> Result<RuntimeConfig>;

    // Docker images with version support
    fn runtime_base_image(&self, version: Option<&str>) -> String;
    fn required_packages(&self) -> Vec<&str>;

    // Start command generation
    fn start_command(&self, entrypoint: &Path) -> String;
}

pub struct RuntimeConfig {
    pub entrypoint: Option<String>,
    pub port: Option<u16>,
    pub env_vars: Vec<String>,
    pub health: Option<HealthCheck>,
    pub native_deps: Vec<String>,
}

pub struct HealthCheck {
    pub endpoint: String,
}
```

**Implementations**:
- `JvmRuntime` - Java/Kotlin
- `NodeRuntime` - JavaScript/TypeScript
- `PythonRuntime` - Python
- `RubyRuntime` - Ruby
- `PhpRuntime` - PHP (PHP-FPM, Laravel Octane, etc.)
- `DotNetRuntime` - C#/F# (.NET runtime)
- `BeamRuntime` - Elixir (Erlang/BEAM VM)
- `NativeRuntime` - Rust/Go/C++ (static binaries)
- `LLMRuntime` - Fallback for unknown runtimes

**Version Handling Example**:
```rust
impl Runtime for NodeRuntime {
    fn runtime_base_image(&self, version: Option<&str>) -> String {
        format!("node:{}-alpine", version.unwrap_or("20"))
    }

    fn required_packages(&self) -> Vec<&str> {
        vec!["dumb-init"]
    }

    fn start_command(&self, entrypoint: &Path) -> String {
        format!("node {}", entrypoint.display())
    }
}
```

### Framework Trait (Already Exists)

**Responsibility**: Framework conventions and defaults

```rust
pub trait Framework: Send + Sync {
    fn name(&self) -> &str;
    fn detect(&self, dependencies: &[Dependency], files: &[PathBuf]) -> bool;
    fn default_ports(&self) -> &[u16];
    fn health_endpoints(&self) -> &[&str];
    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)>;
    fn customize_build_template(&self, template: BuildTemplate) -> BuildTemplate;
}
```

**No changes needed** - already exists, just needs better usage in pipeline.

### BuildSystem Trait (Already Exists)

**No changes needed** - already provides `build_template()` with build and runtime base images.

### MonorepoOrchestrator Trait Extension

```rust
pub trait MonorepoOrchestrator: Send + Sync {
    // ... existing methods ...

    // NEW: Workspace structure parsing
    fn workspace_structure(&self, repo_path: &Path) -> Result<WorkspaceStructure>;

    // NEW: Build order calculation
    fn build_order(&self, workspace: &WorkspaceStructure) -> Vec<PathBuf>;

    // NEW: Workspace-aware build command
    fn build_command(&self, package: &Package, workspace: &WorkspaceStructure) -> String;
}

pub struct WorkspaceStructure {
    pub orchestrator: OrchestratorId,
    pub applications: Vec<Application>,
    pub libraries: Vec<Library>,
    pub build_order: Vec<PathBuf>,
    pub dependency_graph: HashMap<PathBuf, Vec<PathBuf>>,
}
```

## Incremental Migration (14 PRs)

### Track 1: Service Phases (PRs 1-8)

#### **PR1: Runtime Trait Infrastructure** (~200 LOC)

**Scope**: Define trait + JvmRuntime implementation

**Files Changed**:
- `src/runtime/mod.rs` (NEW)
- `src/runtime/jvm.rs` (NEW)

**Changes**:
- Create `Runtime` trait with deterministic + LLM methods
- Define `RuntimeConfig` and `HealthCheck` structs
- Implement `JvmRuntime` with all methods (including Wolfi - unused)
- Unit tests for JvmRuntime

**Value**: Trait exists, can be reviewed independently
**Tests**: Unit tests only
**Phases**: No changes (just adds new code)

---

#### **PR2: Complete Runtime Implementations** (~300 LOC)

**Depends on**: PR1

**Files Changed**:
- `src/runtime/node.rs` (NEW)
- `src/runtime/python.rs` (NEW)
- `src/runtime/ruby.rs` (NEW)
- `src/runtime/php.rs` (NEW)
- `src/runtime/dotnet.rs` (NEW)
- `src/runtime/beam.rs` (NEW)
- `src/runtime/native.rs` (NEW)
- `src/runtime/llm.rs` (NEW)

**Changes**:
- Implement `NodeRuntime`, `PythonRuntime`, `RubyRuntime`, `PhpRuntime`, `DotNetRuntime`, `BeamRuntime`, `NativeRuntime`, `LLMRuntime`
- Unit tests for each

**Value**: All runtime implementations complete
**Tests**: Unit tests per implementation
**Phases**: No changes

---

#### **PR3: Add Health Endpoint to Schema** (~100 LOC)

**Independent** (can run in parallel with PR1-2)

**Files Changed**:
- `src/output/schema.rs`
- `tests/fixtures/expected/*.json`

**Changes**:
- Add `HealthCheck` struct to schema
- Add `health: Option<HealthCheck>` to `RuntimeStage`
- Update validation
- Add `"health": null` to all test fixtures

**Value**: Schema ready for health endpoint
**Tests**: Schema validation tests
**Phases**: No changes

---

#### **PR4: RuntimeConfigPhase Integration** (~200 LOC)

**Depends on**: PR1, PR2

**Files Changed**:
- `src/pipeline/phases/07_runtime_config.rs` (NEW)
- `src/pipeline/phases/07_service_analysis.rs`
- `src/pipeline/service_context.rs`

**Changes**:
- Create `RuntimeConfigPhase`
- Update `ServiceAnalysisPhase` phase list:
  ```rust
  let phases: Vec<&dyn ServicePhase> = vec![
      &RuntimePhase,
      &BuildPhase,
      &RuntimeConfigPhase,  // NEW
      // Old phases disabled but not deleted
      // &EntrypointPhase,
      // &PortPhase,
      // &EnvVarsPhase,
      // &HealthPhase,
      // &NativeDepsPhase,
      &CachePhase,
  ];
  ```
- Update `ServiceContext` to store `RuntimeConfig`
- Wire runtime detection from `RuntimePhase`

**Value**: New phase working
**Tests**: All e2e tests pass (same output, different implementation)
**Phases**: 8 service phases (old disabled, new enabled)

---

#### **PR5: Remove Old Service Phases** (~100 LOC - deletions)

**Depends on**: PR4

**Files Changed**:
- `src/pipeline/phases/07_3_entrypoint.rs` (DELETE)
- `src/pipeline/phases/07_5_port.rs` (DELETE)
- `src/pipeline/phases/07_6_env_vars.rs` (DELETE)
- `src/pipeline/phases/07_7_health.rs` (DELETE)
- `src/pipeline/phases/07_4_native_deps.rs` (DELETE)
- `src/pipeline/phases/07_service_analysis.rs`
- `src/pipeline/service_context.rs`

**Changes**:
- Delete 5 old phase files
- Clean up `ServiceAnalysisPhase` phase list
- Remove old fields from `ServiceContext`

**Value**: Code cleanup
**Tests**: All e2e tests still pass
**Phases**: 8 → 4 service phases

---

#### **PR6: Use Framework Defaults in Runtime** (~150 LOC)

**Depends on**: PR4

**Files Changed**:
- `src/runtime/*.rs` (all implementations)
- `src/pipeline/phases/07_runtime_config.rs`

**Changes**:
- Update all Runtime implementations to use framework defaults:
  ```rust
  let port = extract_port_from_code(files)
      .or_else(|| framework.and_then(|f| f.default_ports().first().copied()))
      .unwrap_or(8080);
  ```
- Update `RuntimeConfigPhase` to pass framework to runtime

**Value**: Spring Boot → 8080, Next.js → 3000 (better defaults)
**Tests**: Verify framework-specific defaults
**Phases**: No change (4 service phases)

---

#### **PR7: Multi-Stage Docker Images** (~100 LOC)

**Depends on**: PR3 (schema)
**Can run in parallel with PR4-PR6**

**Files Changed**:
- `src/pipeline/phases/08_assemble.rs`

**Changes**:
- Read `build_template().build_image` and `build_template().runtime_image`
- Populate `BuildStage.base` and `RuntimeStage.base` separately
- Populate `RuntimeStage.health` from RuntimeConfig (if PR4 done)

**Value**: Multi-stage Docker builds
**Tests**: Verify build/runtime images different
**Phases**: No change

---

#### **PR8: Unified Stack Identification** (~300 LOC)

**Depends on**: PR6

**Files Changed**:
- `src/pipeline/phases/07_0_stack.rs` (NEW - rename from 07_1_runtime.rs)
- `src/pipeline/phases/07_1_runtime.rs` → delete or rename

**Changes**:
- Create `StackIdentificationPhase`:
  ```rust
  // Detect complete stack together
  let language = detect_language(manifest);
  let version = language.detect_version(manifest_content);
  let build_system = detect_build_system(manifest);
  let framework = detect_framework(dependencies);
  let runtime = get_runtime_for_language(language);

  Stack { language, build_system, framework, runtime, version }
  ```
- Update `RuntimePhase` → remove detection logic, use pre-detected stack
- Move framework/language detection out of RuntimePhase

**Value**: Coherent stack detection
**Tests**: All e2e tests pass
**Phases**: Still 4 service phases (StackIdentificationPhase + RuntimeConfigPhase + BuildPhase + CachePhase)

---

### Track 2: Workflow Phases (PRs 9-14)

#### **PR9: MonorepoOrchestrator Trait Extension** (~150 LOC)

**Independent**

**Files Changed**:
- `src/stack/orchestrator/mod.rs`

**Changes**:
- Add `WorkspaceStructure` struct
- Extend `MonorepoOrchestrator` trait (add 3 new methods)
- Add default panic implementations:
  ```rust
  fn workspace_structure(&self, _repo_path: &Path) -> Result<WorkspaceStructure> {
      unimplemented!("Not yet implemented")
  }
  ```

**Value**: Trait contract defined
**Tests**: Trait compiles
**Phases**: No changes

---

#### **PR10: Implement TurborepoOrchestrator** (~250 LOC)

**Depends on**: PR9

**Files Changed**:
- `src/stack/orchestrator/turborepo.rs`

**Changes**:
- Implement `workspace_structure()` (parse turbo.json)
- Implement `build_order()` (topological sort)
- Implement `build_command()` (`turbo run build --filter=app`)
- Integration tests

**Value**: Turbo monorepos work
**Tests**: Turborepo fixture validates workspace
**Phases**: No changes

---

#### **PR11: Implement Nx + Lerna Orchestrators** (~400 LOC)

**Depends on**: PR9

**Files Changed**:
- `src/stack/orchestrator/nx.rs`
- `src/stack/orchestrator/lerna.rs`

**Changes**:
- Implement Nx methods (parse nx.json + project.json)
- Implement Lerna methods (parse lerna.json)
- Integration tests for both

**Value**: Nx and Lerna monorepos work
**Tests**: Fixtures validate both
**Phases**: No changes

---

#### **PR12: WorkspaceStructurePhase Integration** (~300 LOC)

**Depends on**: PR10, PR11

**Files Changed**:
- `src/pipeline/phases/02_workspace.rs` (NEW)
- `src/pipeline/orchestrator.rs`

**Changes**:
- Create `WorkspaceStructurePhase`:
  ```rust
  let orchestrator = detect_orchestrator(repo_path);
  let workspace = if orchestrator.is_some() {
      orchestrator.workspace_structure(repo_path)?
  } else {
      single_project_structure(scan)?
  };
  ```
- Update pipeline orchestrator:
  ```rust
  let workflow_phases: Vec<Box<dyn WorkflowPhase>> = vec![
      Box::new(ScanPhase),
      Box::new(WorkspaceStructurePhase),  // NEW
      // Old phases disabled
      // Box::new(ClassifyPhase),
      // Box::new(StructurePhase),
      Box::new(DependenciesPhase),
      Box::new(BuildOrderPhase),
      Box::new(RootCachePhase),
      Box::new(ServiceAnalysisPhase),
      Box::new(AssemblePhase),
  ];
  ```

**Value**: New workspace phase working
**Tests**: All e2e tests pass (monorepo tests validate workspace)
**Phases**: 8 workflow phases (old disabled)

---

#### **PR13: Remove Classify + Structure Phases** (~100 LOC)

**Depends on**: PR12

**Files Changed**:
- `src/pipeline/phases/02_classify.rs` (DELETE)
- `src/pipeline/phases/03_structure.rs` (DELETE)
- `src/pipeline/context.rs`

**Changes**:
- Delete old phase files
- Remove old fields from `AnalysisContext`

**Value**: Code cleanup
**Tests**: All e2e tests still pass
**Phases**: 8 → 6 workflow phases

---

#### **PR14: Merge Dependencies into WorkspaceStructure** (~200 LOC)

**Depends on**: PR12

**Files Changed**:
- `src/pipeline/phases/02_workspace.rs`
- `src/pipeline/phases/04_dependencies.rs` (DELETE)
- `src/pipeline/phases/05_build_order.rs` (DELETE)
- `src/stack/orchestrator/*.rs`

**Changes**:
- Move dependency graph into `WorkspaceStructurePhase`
- Update `MonorepoOrchestrator.workspace_structure()` to include dependencies
- Delete `DependenciesPhase` and `BuildOrderPhase`

**Value**: Final pipeline
**Tests**: All e2e tests pass
**Phases**: 6 → 5 workflow phases

---

## Final Pipeline State

**After all 14 PRs**:

```
Workflow Phases (5):
1. ScanPhase
2. WorkspaceStructurePhase  (uses MonorepoOrchestrator)
3. RootCachePhase
4. ServiceAnalysisPhase
5. AssemblePhase

Service Phases (4):
1. StackIdentificationPhase  (Language + BuildSystem + Framework + Runtime)
2. BuildPhase
3. RuntimeConfigPhase        (uses Runtime + Framework)
4. CachePhase
```

**Total: 5 workflow + 4 service = 9 phases** (down from 16)

## Version Handling

**Flow**:
1. `StackIdentificationPhase` detects version via `Language.detect_version(manifest)`
2. Stores in `Stack { runtime, version: Option<String> }`
3. `AssemblePhase` calls `runtime.runtime_base_image(stack.version.as_deref())`

**Example (Node.js)**:
```
package.json: { "engines": { "node": "20.x" } }
  ↓
Language.detect_version() → "20.x"
  ↓
Stack { runtime: NodeRuntime, version: Some("20.x") }
  ↓
runtime.runtime_base_image(Some("20.x")) → "node:20-alpine"
```

## UniversalBuild Schema Changes

### Add Health Endpoint Field

**Current RuntimeStage**:
```rust
pub struct RuntimeStage {
    pub base: String,
    pub packages: Vec<String>,
    pub env: HashMap<String, String>,
    pub copy: Vec<CopySpec>,
    pub command: Vec<String>,
    pub ports: Vec<u16>,
}
```

**Updated RuntimeStage**:
```rust
pub struct RuntimeStage {
    pub base: String,
    pub packages: Vec<String>,
    pub env: HashMap<String, String>,
    pub copy: Vec<CopySpec>,
    pub command: Vec<String>,
    pub ports: Vec<u16>,
    pub health: Option<HealthCheck>,  // NEW
}

pub struct HealthCheck {
    pub endpoint: String,
}
```

**Applied in**: PR3

## UniversalBuild Field Ownership Analysis

Each field in `UniversalBuild` originates from one or more knowledge domains:

### Metadata Section

| Field          | Origin           | Provider                                      |
|----------------|------------------|-----------------------------------------------|
| `project_name` | **Build System** | Manifest (package.json name, Cargo.toml name) |
| `language`     | **Language**     | LanguageDefinition.detect()                   |
| `build_system` | **Build System** | BuildSystem.detect()                          |
| `framework`    | **Framework**    | StackRegistry.detect_framework()              |
| `confidence`   | **Analysis**     | Aggregated from all detections                |
| `reasoning`    | **Analysis**     | Why these decisions were made                 |

### BuildStage Section

| Field       | Origin                                              | How They Collaborate                                                                                                                                                                                    |
|-------------|-----------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `base`      | **Build System**                                    | BuildSystem provides build image (e.g., `rust:1.75-alpine`, `node:20-alpine`, `maven:3.9-jdk-21`)                                                                                                       |
| `packages`  | **Build System** + **Language**                     | BuildSystem knows build tools needed (e.g., `pkg-config`), Language adds language-specific build deps (e.g., `gcc` for C extensions in Python)                                                          |
| `env`       | **Build System** + **Framework**                    | BuildSystem sets tool paths (`CARGO_HOME=/cache`), Framework adds build-time vars (e.g., Next.js `NODE_ENV=production` for optimized build)                                                             |
| `commands`  | **Build System** + **Framework** + **Orchestrator** | **Priority chain**: Orchestrator wraps if monorepo (`turbo run build --filter=app`) → Framework overrides if custom steps (`npm run build` for Next.js) → BuildSystem default (`cargo build --release`) |
| `context`   | **Build System**                                    | BuildSystem knows what files to copy for build (manifest, source code, lockfiles)                                                                                                                       |
| `cache`     | **Build System** + **Orchestrator**                 | BuildSystem provides tool cache (`target/`, `node_modules/`), Orchestrator adds workspace cache (`.turbo/`, `.nx/`) - **concatenated**                                                                  |
| `artifacts` | **Build System**                                    | BuildSystem knows output location (`target/release/app`, `dist/`, `.next/standalone/`)                                                                                                                  |

### RuntimeStage Section

| Field      | Origin                      | How They Collaborate                                                                                                                                                                                                                                                                                                                        |
|------------|-----------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `base`     | **Runtime**                 | Runtime provides platform image (e.g., JVM: `eclipse-temurin:21-jre-alpine`, Node: `node:20-alpine`, Native: `alpine:latest` or `scratch`)                                                                                                                                                                                                  |
| `packages` | **Runtime** + **Framework** | Runtime provides platform deps (JVM: `ca-certificates`, Node: `dumb-init`), Framework adds framework deps (Spring Boot: `curl` for health checks) - **concatenated**                                                                                                                                                                        |
| `env`      | **Runtime** + **Framework** | **Merge strategy**: Runtime sets platform defaults (Node: `NODE_ENV=production`, JVM: `JAVA_TOOL_OPTIONS=-XX:+UseContainerSupport`) → Framework adds conventions (Spring Boot: `SPRING_PROFILES_ACTIVE=prod`) → Code extraction adds app-specific vars (`DATABASE_URL`, `API_KEY`) - **all merged, code extraction takes final precedence** |
| `copy`     | **Build System**            | BuildSystem knows which artifacts to copy from build stage (`target/release/app`, `dist/`, `.next/standalone/`)                                                                                                                                                                                                                             |
| `command`  | **Runtime** + **Framework** | **Fallback chain**: Framework provides hint if exists (Spring Boot: `java -jar app.jar`) → Runtime applies conventions (JVM: adds JVM flags, Node: adds `node`, Native: runs binary directly) → Final: `java -XX:MaxRAMPercentage=75.0 -jar app.jar`                                                                                        |
| `ports`    | **Framework** + **Runtime** | **Priority**: Code extraction first (scan for `server.listen(3000)`) → Framework default if not found (Spring Boot: `8080`, Next.js: `3000`) → Generic fallback `8080`                                                                                                                                                                      |
| `health`   | **Framework** + **Runtime** | **Priority**: Code extraction first (scan route definitions) → Framework default if not found (Spring Boot: `/actuator/health`, Rails: `/up`) → None if not detected (NEW)                                                                                                                                                                  |

### Key Insight: Collaborative Fields

Most fields are **collaborative** - they come from multiple domains:

- **`build.commands`**: Build System → Framework override → Orchestrator wraps
- **`runtime.command`**: Runtime convention → Framework hints → Final command
- **`runtime.ports`**: Framework default → Code extraction → Fallback
- **`runtime.env`**: Runtime defaults + Framework conventions + Code extraction
- **`runtime.health`**: Framework endpoint → Code extraction → Fallback

This validates the trait collaboration design:
```rust
runtime.extract_config(files, framework)        // Runtime uses Framework
orchestrator.build_command(package)             // Orchestrator wraps build system
framework.customize_build_template(template)    // Framework customizes build
```

## Testing Strategy

Each PR:
- ✅ All e2e tests pass
- ✅ Add new unit tests for trait implementations
- ✅ UniversalBuild schema changes: PR3 (health endpoint)
- ✅ No feature flags or parallel implementations

## Dependencies on Other OpenSpec Changes

This change is **independent** and can be completed in any order relative to other pending changes.
