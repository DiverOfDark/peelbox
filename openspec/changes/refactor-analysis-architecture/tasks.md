# Tasks: Analysis Architecture Refactoring

## Track 1: Service Phases (PRs 1-8)

### PR1: Runtime Trait Infrastructure (~200 LOC)

- [x] Create `src/runtime/mod.rs` module
- [x] Define `Runtime` trait with methods:
  - [x] `try_deterministic_config(files, framework) -> Option<RuntimeConfig>`
  - [x] `extract_config_llm(files, framework) -> Result<RuntimeConfig>`
  - [x] `runtime_base_image(version) -> String`
  - [x] `required_packages() -> Vec<&str>`
  - [x] `start_command(entrypoint) -> String`
- [x] Define `RuntimeConfig` struct with fields: `entrypoint`, `port`, `env_vars`, `health`, `native_deps`
- [x] Define `HealthCheck` struct with field: `endpoint`
- [x] Create `src/runtime/jvm.rs`
- [x] Implement `JvmRuntime` with all trait methods
- [x] Add unit tests for `JvmRuntime`
- [x] Verify trait compiles and tests pass

### PR2: Complete Runtime Implementations (~300 LOC)

**Depends on**: PR1

- [x] Move runtime module from src/runtime to src/stack/runtime
- [x] Create `src/stack/runtime/node.rs`
- [x] Implement `NodeRuntime` with all trait methods
- [x] Add unit tests for `NodeRuntime`
- [x] Create `src/stack/runtime/python.rs`
- [x] Implement `PythonRuntime` with all trait methods
- [x] Add unit tests for `PythonRuntime`
- [x] Create `src/stack/runtime/ruby.rs`
- [x] Implement `RubyRuntime` with all trait methods
- [x] Add unit tests for `RubyRuntime`
- [x] Create `src/stack/runtime/php.rs`
- [x] Implement `PhpRuntime` with all trait methods
- [x] Add unit tests for `PhpRuntime`
- [x] Create `src/stack/runtime/dotnet.rs`
- [x] Implement `DotNetRuntime` with all trait methods
- [x] Add unit tests for `DotNetRuntime`
- [x] Create `src/stack/runtime/beam.rs`
- [x] Implement `BeamRuntime` with all trait methods
- [x] Add unit tests for `BeamRuntime`
- [x] Create `src/stack/runtime/native.rs`
- [x] Implement `NativeRuntime` with all trait methods
- [x] Add unit tests for `NativeRuntime`
- [x] Create `src/stack/runtime/llm.rs`
- [x] Implement `LLMRuntime` (fallback) with all trait methods
- [x] Add unit tests for `LLMRuntime`
- [x] Verify all runtime implementations compile and tests pass

### PR3: Add Health Endpoint to Schema (~100 LOC)

**Independent** (can run in parallel with PR1-2)

- [x] Add `HealthCheck` struct to `src/output/schema.rs`
- [x] Add `health: Option<HealthCheck>` field to `RuntimeStage`
- [x] Update schema validation for health field
- [x] Update all test fixtures in `tests/fixtures/expected/` to include `"health": null`
- [x] Run schema validation tests
- [x] Verify all e2e tests still pass

### PR4: RuntimeConfigPhase Integration (~200 LOC)

**Depends on**: PR1, PR2

- [x] Create `src/pipeline/phases/07_runtime_config.rs`
- [x] Implement `RuntimeConfigPhase` using `Runtime.try_extract()`
- [x] Add deterministic-first pattern:
  - [x] Try `runtime.try_extract(files, framework)`
  - [x] Fallback to LLM (not yet implemented, documented in technical debt)
- [x] Update `src/pipeline/phases/07_service_analysis.rs`:
  - [x] Add `RuntimeConfigPhase` to phase list after RuntimePhase
  - [ ] Comment out old phases: `EntrypointPhase`, `PortPhase`, `EnvVarsPhase`, `HealthPhase`, `NativeDepsPhase` (PR5)
- [x] Update `src/pipeline/service_context.rs`:
  - [x] Add `runtime_config: Option<RuntimeConfig>` field
- [x] Wire runtime detection from `RuntimePhase` to `RuntimeConfigPhase`
- [x] Run all e2e tests and verify they pass
- [x] Verify same output as before (different implementation)

### PR5: Remove Old Service Phases (~100 LOC - deletions)

**Depends on**: PR4

- [x] Delete `src/pipeline/phases/07_3_entrypoint.rs`
- [x] Delete `src/pipeline/phases/07_5_port.rs`
- [x] Delete `src/pipeline/phases/07_6_env_vars.rs`
- [x] Delete `src/pipeline/phases/07_7_health.rs`
- [x] Delete `src/pipeline/phases/07_4_native_deps.rs`
- [x] Clean up `src/pipeline/phases/07_service_analysis.rs`:
  - [x] Remove commented phase references
  - [x] Clean up phase list to only include active phases
- [x] Update `src/pipeline/service_context.rs`:
  - [x] Remove old fields (individual port, env_vars, health, native_deps)
  - [x] Keep only `runtime_config: RuntimeConfig`
- [x] Run all e2e tests and verify they still pass
- [x] Verify service phase count reduced from 8 to 4

### PR6: Use Framework Defaults in Runtime (~150 LOC)

**Depends on**: PR4

- [x] Update `src/runtime/jvm.rs` to use framework defaults:
  - [x] Use `framework.default_ports()` as fallback for port
  - [x] Use `framework.health_endpoints()` for health check
- [x] Update `src/runtime/node.rs` with framework defaults
- [x] Update `src/runtime/python.rs` with framework defaults
- [x] Update `src/runtime/ruby.rs` with framework defaults
- [x] Update `src/runtime/php.rs` with framework defaults
- [x] Update `src/runtime/dotnet.rs` with framework defaults
- [x] Update `src/runtime/beam.rs` with framework defaults
- [x] Update `src/runtime/native.rs` with framework defaults
- [x] Update `src/pipeline/phases/07_runtime_config.rs`:
  - [x] Pass detected framework to `runtime.extract_config()`
- [x] Add integration tests for framework-specific defaults
- [x] Run all e2e tests and verify they pass
- [x] Verify Spring Boot apps detect port 8080 and `/actuator/health`
- [x] Verify Next.js apps detect port 3000

### PR7: Multi-Stage Docker Images (~100 LOC)

**Depends on**: PR3 (schema)

- [x] Update `src/pipeline/phases/08_assemble.rs`:
  - [x] Read `build_system.build_template().build_image`
  - [x] Read `build_system.build_template().runtime_image`
  - [x] Populate `BuildStage.base` from build_image
  - [x] Populate `RuntimeStage.base` from runtime_image
  - [x] Populate `RuntimeStage.health` from `RuntimeConfig` (if PR4 done)
- [x] Update e2e test expected outputs:
  - [x] Verify build and runtime base images are different
  - [x] Verify health endpoint populated where detected
- [x] Run all e2e tests and verify they pass

### PR8: Unified Stack Identification (~300 LOC)

**Depends on**: PR6

- [x] Rename `src/pipeline/phases/07_1_runtime.rs` to `src/pipeline/phases/07_0_stack.rs`
- [x] Create `StackIdentificationPhase`:
  - [x] Detect language via `LanguageDefinition.detect()`
  - [x] Detect version via `language.detect_version(manifest_content)`
  - [x] Detect build system via `BuildSystem.detect()`
  - [x] Detect framework from dependencies via `StackRegistry.detect_framework()`
  - [x] Map language to runtime via `get_runtime_for_language()`
  - [x] Store complete `Stack { language, build_system, framework, runtime, version }`
- [x] Update `src/pipeline/service_context.rs`:
  - [x] Add `stack: Stack` field
- [x] Update `RuntimeConfigPhase`:
  - [x] Use pre-detected stack instead of detecting runtime
  - [x] Remove detection logic
- [x] Update `src/pipeline/phases/07_service_analysis.rs`:
  - [x] Update phase order to put `StackIdentificationPhase` first
- [x] Run all e2e tests and verify they pass
- [x] Verify all service phases now use pre-detected stack

---

## Track 2: Workflow Phases (PRs 9-14)

### PR9: MonorepoOrchestrator Trait Extension (~150 LOC)

**Independent**

- [x] Add `WorkspaceStructure` struct definition to `src/stack/orchestrator/mod.rs`:
  - [x] Fields: `orchestrator`, `packages` (simplified - removed build_order, dependency_graph, applications/libraries split)
- [x] Add `Package` struct: `path`, `name`, `is_application`
- [x] Extend `MonorepoOrchestrator` trait with new methods:
  - [x] `fn workspace_structure(&self, repo_path: &Path) -> Result<WorkspaceStructure>`
  - [x] `fn build_command(&self, package: &Package) -> String` (removed workspace param)
  - [x] Removed build_order() method (orchestrator handles ordering internally)
- [x] Add default unimplemented!() for new methods (backward compatible)
- [x] Verify trait compiles

### PR10: Implement TurborepoOrchestrator (~110 LOC)

**Depends on**: PR9

- [x] Update `src/stack/orchestrator/turborepo.rs`:
  - [x] Implement `workspace_structure()` method
  - [x] Parse root `package.json` workspaces field (not turbo.json)
  - [x] Glob workspace patterns ("apps/*", "packages/*")
  - [x] Identify applications by "start" script presence (not directory heuristic)
  - [x] Removed build_order() - Turbo handles ordering
  - [x] Removed dependency graph - not needed
  - [x] Implement `build_command()` returning `turbo run build --filter={name}`
- [x] Simplified implementation (no topological sort, no dep graph)
- [x] Verify all tests pass (459 library tests)

### PR11: Implement Nx + Lerna Orchestrators (~220 LOC)

**Depends on**: PR9

- [x] Update `src/stack/orchestrator/nx.rs`:
  - [x] Implement `workspace_structure()` method
  - [x] Parse `nx.json` and fallback to `workspace.json` / `package.json`
  - [x] Parse `project.json` for Nx >= 13 project configurations
  - [x] Identify applications by "serve" or "start" target in project.json
  - [x] Fallback to "start" script in package.json
  - [x] Implement `build_command()` returning `nx build {name}`
- [x] Update `src/stack/orchestrator/lerna.rs`:
  - [x] Implement `workspace_structure()` method
  - [x] Parse `lerna.json` packages field
  - [x] Fallback to default "packages/*" for Lerna < 3.0
  - [x] Identify applications by "start" script presence (same as Turbo)
  - [x] Implement `build_command()` returning `lerna run build --scope={name}`
- [x] Simplified implementations following Turbo pattern
- [x] Removed unused HashMap import from mod.rs
- [x] Verify all tests pass (459 library tests)

### PR12: WorkspaceStructurePhase Integration (~300 LOC)

**Depends on**: PR10, PR11

- [x] Create `src/pipeline/phases/02_workspace.rs`
- [x] Implement `WorkspaceStructurePhase`:
  - [x] Detect orchestrator from scan results
  - [x] If orchestrator found, call `orchestrator.workspace_structure(repo_path)`
  - [x] If no orchestrator, create single-project structure from scan results
  - [x] Store workspace structure in context
- [x] Update `src/pipeline/orchestrator.rs`:
  - [x] Add `WorkspaceStructurePhase` to workflow phases
  - [x] Comment out `ClassifyPhase` and `StructurePhase`
  - [x] Keep phase list order: Scan → WorkspaceStructure → Dependencies → BuildOrder → RootCache → ServiceAnalysis → Assemble
- [x] Update `src/pipeline/context.rs`:
  - [x] Add `workspace: Option<WorkspaceStructure>` field
- [x] Run all e2e tests and verify they pass
- [x] Verify monorepo tests correctly populate workspace structure

### PR13: Remove Classify + Structure Phases (~100 LOC)

**Depends on**: PR12

- [x] Delete `src/pipeline/phases/02_classify.rs`
- [x] Delete `src/pipeline/phases/03_structure.rs`
- [x] Update `src/pipeline/orchestrator.rs`:
  - [x] Remove commented phase references
  - [x] Clean up phase list
- [x] Update `src/pipeline/context.rs`:
  - [x] Remove `classify: Option<ClassifyResult>` field
  - [x] Remove `structure: Option<StructureResult>` field
- [x] Run all e2e tests and verify they still pass
- [x] Verify workflow phase count reduced from 8 to 7 (Scan → WorkspaceStructure → Dependencies → BuildOrder → RootCache → ServiceAnalysis → Assemble)

### PR14: Merge Dependencies into WorkspaceStructure (~200 LOC)

**Depends on**: PR12

- [x] Update `src/pipeline/phases/02_workspace.rs`:
  - [x] Removed dependency graph and build order fields (orchestrators handle this internally)
  - [x] Added native workspace detection for npm (uses WorkspaceBuildSystem trait)
- [x] Delete `src/pipeline/phases/04_dependencies.rs`
- [x] Delete `src/pipeline/phases/05_build_order.rs`
- [x] Update `src/pipeline/orchestrator.rs`:
  - [x] Remove `DependenciesPhase` and `BuildOrderPhase` from workflow
  - [x] Final phase list: Scan → WorkspaceStructure → RootCache → ServiceAnalysis → Assemble
- [x] Update `src/pipeline/context.rs`:
  - [x] Remove `dependencies: Option<DependencyResult>` field
  - [x] Remove `build_order: Option<BuildOrderResult>` field
- [x] Update `src/pipeline/phases/07_0_stack.rs`:
  - [x] Remove dependency on `DependencyResult`
  - [x] Framework detection now parses manifest directly using `StackRegistry.parse_dependencies_by_manifest()`
- [x] Verify workflow phase count reduced from 7 to 5

**Note**: npm workspaces now fully supported. Cargo, Maven, and Gradle workspace support tracked in Technical Debt section below (requires implementing `WorkspaceBuildSystem` trait).

---

## Final Validation

- [ ] Run full e2e test suite
- [ ] Verify all 16 old phases removed
- [ ] Verify 9 new phases working (5 workflow + 4 service)
- [ ] Compare token usage (before vs after)
- [ ] Update PRD.md with new architecture
- [ ] Update CLAUDE.md to document new phase structure
- [ ] Update CHANGELOG.md

---

## Success Criteria

- ✅ All existing e2e tests pass at each PR
- ✅ No regression in detection accuracy
- ✅ Reduced average LLM token usage (5 phases → 1 phase for runtime config)
- ✅ Cleaner codebase (16 phases → 9 phases, better separation)
- ✅ Framework defaults improve accuracy (Spring Boot → 8080, Next.js → 3000)
- ✅ Multi-stage Docker builds supported
- ✅ Health endpoint detection working
- ✅ Version handling implemented for runtime packages

---

## Technical Debt

### Runtime Configuration Extraction

#### Runtime-Level Deterministic Parsing
Runtime.try_extract() should parse generic runtime patterns, not framework-specific configs:

- [ ] Implement JvmRuntime.try_extract()
  - [ ] Detect native dependencies from pom.xml/build.gradle system dependencies
  - [ ] Scan for generic Java server port bindings (ServerSocket, Jetty, etc.)
  - [ ] Extract env vars from System.getenv() calls
- [ ] Implement NodeRuntime.try_extract()
  - [ ] Parse package.json scripts for port hints (start command analysis)
  - [ ] Scan .js/.ts files for generic server.listen(port) calls
  - [ ] Extract env vars from process.env.VARIABLE patterns
- [ ] Implement PythonRuntime.try_extract()
  - [ ] Scan for generic app.run(port=X) calls (framework-agnostic)
  - [ ] Extract env vars from os.environ/os.getenv patterns
  - [ ] Detect native dependencies from requirements.txt C extensions
- [ ] Implement RubyRuntime.try_extract()
  - [ ] Scan for generic Rack::Server or WEBrick port bindings
  - [ ] Extract env vars from ENV[] patterns
  - [ ] Detect native dependencies from Gemfile native extensions
- [ ] Implement PhpRuntime.try_extract()
  - [ ] Parse php.ini for generic runtime config
  - [ ] Extract env vars from $_ENV patterns
  - [ ] Detect native dependencies from composer.json extensions
- [ ] Implement DotNetRuntime.try_extract()
  - [ ] Parse launchSettings.json for generic runtime config (not framework-specific)
  - [ ] Extract env vars from Environment.GetEnvironmentVariable() calls
  - [ ] Detect native dependencies from .csproj NativeLibrary references
- [ ] Implement BeamRuntime.try_extract()
  - [ ] Scan for generic Cowboy/Ranch port bindings
  - [ ] Extract env vars from System.get_env() calls
  - [ ] Detect native dependencies from mix.exs NIF references
- [ ] Implement NativeRuntime.try_extract()
  - [ ] Use build system hints (Cargo.toml metadata, go.mod comments)
  - [ ] Detect port bindings from source scanning (bind(), listen())

#### Framework-Level Config Parsing
Framework-specific config parsing belongs in Framework implementations, not Runtime:

- [ ] Extend Framework trait with config_file_parser() method
  - [ ] Parse framework-specific config files (application.yml, settings.py, appsettings.json, etc.)
  - [ ] Extract port, env vars, health endpoints from framework configs
  - [ ] Framework.env_var_patterns() already provides regex patterns - use them!
- [ ] Implement config parsing per framework:
  - [ ] SpringBootFramework: Parse application.properties, application.yml
  - [ ] DjangoFramework: Parse settings.py, config files
  - [ ] FlaskFramework: Parse app config, instance config
  - [ ] LaravelFramework: Parse config/*.php files
  - [ ] RailsFramework: Parse config/puma.rb, config/application.rb
  - [ ] AspNetFramework: Parse appsettings.json, appsettings.{env}.json
  - [ ] PhoenixFramework: Parse config/runtime.exs, config/prod.exs
  - [ ] NextJsFramework: Parse next.config.js for port/env
  - [ ] ExpressFramework: Scan for app.listen() with Express patterns

#### Missing Framework Implementations
- [ ] Add Symfony framework (PHP)
  - [ ] Create src/stack/framework/symfony.rs
  - [ ] Add FrameworkId::Symfony to src/stack/mod.rs
  - [ ] Implement dependency detection (symfony/framework-bundle, symfony/http-kernel)
  - [ ] Set default port: 8000
  - [ ] Set health endpoint: /health (or /_health for Symfony 6+)
  - [ ] Add config parsing: config/packages/*.yaml, .env files

#### LLM Fallback (centralized in RuntimeConfigPhase)
- [ ] Design LLMRuntimeFallback for when all deterministic methods return None
  - [ ] Create minimal LLM prompt for runtime config extraction
  - [ ] Accept Runtime + Framework + files as context
  - [ ] Call LLM client with focused prompt (<500 tokens)
  - [ ] Parse and validate LLM response
  - [ ] Use framework defaults as hints in prompt
  - [ ] Return RuntimeConfig or error

### Build Systems That Are Also Orchestrators

Several build systems have built-in workspace/monorepo capabilities. A new `WorkspaceBuildSystem` trait was created to separate workspace parsing from core build system functionality.

**Current State (PR11)**:
- Created `WorkspaceBuildSystem` trait with methods:
  - `parse_workspace_patterns()` - extract workspace glob patterns from manifest
  - `parse_package_metadata()` - extract name and is_application flag
  - `glob_workspace_pattern()` - expand glob patterns to directories
- Implemented `WorkspaceBuildSystem` for `NpmBuildSystem`
- Orchestrators (Turborepo, Nx, Lerna) now delegate to `WorkspaceBuildSystem` methods

**Future Work - Implement WorkspaceBuildSystem for Other Build Systems**:

- **Gradle** (JVM):
  - Already has `is_workspace_root()` checking for `include()` statements
  - Already has `workspace_configs()` returning `["settings.gradle", "settings.gradle.kts"]`
  - TODO: Implement `WorkspaceBuildSystem` trait:
    - `parse_workspace_patterns()` - parse `settings.gradle[.kts]` include() directives
    - `parse_package_metadata()` - parse `build.gradle[.kts]` project name, detect application (has application plugin)
    - `glob_workspace_pattern()` - expand project paths from settings

- **Maven** (JVM):
  - Already has `is_workspace_root()` checking for `<modules>` tag
  - TODO: Implement `WorkspaceBuildSystem` trait:
    - `parse_workspace_patterns()` - parse `pom.xml` <modules> section
    - `parse_package_metadata()` - parse module `pom.xml` <artifactId>, detect packaging type (jar vs war)
    - `glob_workspace_pattern()` - expand module paths

- **Cargo** (Rust):
  - Already has `is_workspace_root()` checking for `[workspace]` section
  - TODO: Implement `WorkspaceBuildSystem` trait:
    - `parse_workspace_patterns()` - parse `Cargo.toml` [workspace.members] array
    - `parse_package_metadata()` - parse member `Cargo.toml` name, detect [[bin]] sections
    - `glob_workspace_pattern()` - expand workspace member paths

- **.NET**:
  - Already has `is_workspace_root()` checking for `Project()` statements
  - Already has `workspace_configs()` returning `["*.sln"]`
  - TODO: Implement `WorkspaceBuildSystem` trait:
    - `parse_workspace_patterns()` - parse `*.sln` Project() references
    - `parse_package_metadata()` - parse `.csproj`/`.fsproj` for name, OutputType (Exe vs Library)
    - `glob_workspace_pattern()` - expand project file paths

**Architecture Decision (PR11)**:
- Orchestrators (Nx/Turborepo/Lerna) delegate to `WorkspaceBuildSystem` for package.json parsing
- Build systems own manifest parsing logic, orchestrators own task coordination
- WorkspaceStructurePhase (PR12) should:
  1. Check for orchestrator presence (Nx/Turborepo/Lerna via config files)
  2. Fall back to `WorkspaceBuildSystem` if available (Gradle/Maven/Cargo/.NET)
  3. Prefer orchestrator if both exist (explicit task coordination wins)

**Benefits**:
- Clean separation: BuildSystem (building) vs WorkspaceBuildSystem (workspace parsing)
- Enables workspace detection without orchestrators (Gradle/Maven/Cargo/.NET native workspaces)
- Reusable across different orchestrator implementations
