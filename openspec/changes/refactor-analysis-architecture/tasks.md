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

- [ ] Update `src/pipeline/phases/08_assemble.rs`:
  - [ ] Read `build_system.build_template().build_image`
  - [ ] Read `build_system.build_template().runtime_image`
  - [ ] Populate `BuildStage.base` from build_image
  - [ ] Populate `RuntimeStage.base` from runtime_image
  - [ ] Populate `RuntimeStage.health` from `RuntimeConfig` (if PR4 done)
- [ ] Update e2e test expected outputs:
  - [ ] Verify build and runtime base images are different
  - [ ] Verify health endpoint populated where detected
- [ ] Run all e2e tests and verify they pass

### PR8: Unified Stack Identification (~300 LOC)

**Depends on**: PR6

- [ ] Rename `src/pipeline/phases/07_1_runtime.rs` to `src/pipeline/phases/07_0_stack.rs`
- [ ] Create `StackIdentificationPhase`:
  - [ ] Detect language via `LanguageDefinition.detect()`
  - [ ] Detect version via `language.detect_version(manifest_content)`
  - [ ] Detect build system via `BuildSystem.detect()`
  - [ ] Detect framework from dependencies via `StackRegistry.detect_framework()`
  - [ ] Map language to runtime via `get_runtime_for_language()`
  - [ ] Store complete `Stack { language, build_system, framework, runtime, version }`
- [ ] Update `src/pipeline/service_context.rs`:
  - [ ] Add `stack: Stack` field
- [ ] Update `RuntimeConfigPhase`:
  - [ ] Use pre-detected stack instead of detecting runtime
  - [ ] Remove detection logic
- [ ] Update `src/pipeline/phases/07_service_analysis.rs`:
  - [ ] Update phase order to put `StackIdentificationPhase` first
- [ ] Run all e2e tests and verify they pass
- [ ] Verify all service phases now use pre-detected stack

---

## Track 2: Workflow Phases (PRs 9-14)

### PR9: MonorepoOrchestrator Trait Extension (~150 LOC)

**Independent**

- [ ] Add `WorkspaceStructure` struct definition to `src/stack/orchestrator/mod.rs`:
  - [ ] Fields: `orchestrator`, `applications`, `libraries`, `build_order`, `dependency_graph`
- [ ] Extend `MonorepoOrchestrator` trait with new methods:
  - [ ] `fn workspace_structure(&self, repo_path: &Path) -> Result<WorkspaceStructure>`
  - [ ] `fn build_order(&self, workspace: &WorkspaceStructure) -> Vec<PathBuf>`
  - [ ] `fn build_command(&self, package: &Package, workspace: &WorkspaceStructure) -> String`
- [ ] Add default panic implementations for existing orchestrators:
  ```rust
  fn workspace_structure(&self, _repo_path: &Path) -> Result<WorkspaceStructure> {
      unimplemented!("Not yet implemented")
  }
  ```
- [ ] Verify trait compiles

### PR10: Implement TurborepoOrchestrator (~250 LOC)

**Depends on**: PR9

- [ ] Update `src/stack/orchestrator/turborepo.rs`:
  - [ ] Implement `workspace_structure()` method
  - [ ] Parse `turbo.json` for workspace configuration
  - [ ] Identify applications vs libraries
  - [ ] Implement `build_order()` method with topological sort
  - [ ] Implement `build_command()` method returning `turbo run build --filter={app}`
- [ ] Add integration tests for TurborepoOrchestrator:
  - [ ] Test workspace parsing
  - [ ] Test build order calculation
  - [ ] Test build command generation
- [ ] Run Turborepo fixture test and verify workspace structure detected
- [ ] Verify all tests pass

### PR11: Implement Nx + Lerna Orchestrators (~400 LOC)

**Depends on**: PR9

- [ ] Update `src/stack/orchestrator/nx.rs`:
  - [ ] Implement `workspace_structure()` method
  - [ ] Parse `nx.json` and `project.json` files
  - [ ] Identify applications vs libraries
  - [ ] Implement `build_order()` method with topological sort
  - [ ] Implement `build_command()` method returning `nx build {app}`
- [ ] Add integration tests for NxOrchestrator
- [ ] Update `src/stack/orchestrator/lerna.rs`:
  - [ ] Implement `workspace_structure()` method
  - [ ] Parse `lerna.json` for workspace configuration
  - [ ] Identify applications vs libraries
  - [ ] Implement `build_order()` method with topological sort
  - [ ] Implement `build_command()` method returning `lerna run build --scope={app}`
- [ ] Add integration tests for LernaOrchestrator
- [ ] Run Nx and Lerna fixture tests
- [ ] Verify all tests pass

### PR12: WorkspaceStructurePhase Integration (~300 LOC)

**Depends on**: PR10, PR11

- [ ] Create `src/pipeline/phases/02_workspace.rs`
- [ ] Implement `WorkspaceStructurePhase`:
  - [ ] Detect orchestrator from scan results
  - [ ] If orchestrator found, call `orchestrator.workspace_structure(repo_path)`
  - [ ] If no orchestrator, create single-project structure from scan results
  - [ ] Store workspace structure in context
- [ ] Update `src/pipeline/orchestrator.rs`:
  - [ ] Add `WorkspaceStructurePhase` to workflow phases
  - [ ] Comment out `ClassifyPhase` and `StructurePhase`
  - [ ] Keep phase list order: Scan → WorkspaceStructure → Dependencies → BuildOrder → RootCache → ServiceAnalysis → Assemble
- [ ] Update `src/pipeline/context.rs`:
  - [ ] Add `workspace: Option<WorkspaceStructure>` field
- [ ] Run all e2e tests and verify they pass
- [ ] Verify monorepo tests correctly populate workspace structure

### PR13: Remove Classify + Structure Phases (~100 LOC)

**Depends on**: PR12

- [ ] Delete `src/pipeline/phases/02_classify.rs`
- [ ] Delete `src/pipeline/phases/03_structure.rs`
- [ ] Update `src/pipeline/orchestrator.rs`:
  - [ ] Remove commented phase references
  - [ ] Clean up phase list
- [ ] Update `src/pipeline/context.rs`:
  - [ ] Remove `classify: Option<ClassifyResult>` field
  - [ ] Remove `structure: Option<StructureResult>` field
- [ ] Run all e2e tests and verify they still pass
- [ ] Verify workflow phase count reduced from 8 to 6

### PR14: Merge Dependencies into WorkspaceStructure (~200 LOC)

**Depends on**: PR12

- [ ] Update `src/pipeline/phases/02_workspace.rs`:
  - [ ] Move dependency graph extraction into `WorkspaceStructurePhase`
  - [ ] Populate `WorkspaceStructure.dependency_graph` field
  - [ ] Populate `WorkspaceStructure.build_order` field
- [ ] Update all orchestrator implementations:
  - [ ] Update `workspace_structure()` to include dependency parsing
  - [ ] Return complete workspace with dependencies and build order
- [ ] Delete `src/pipeline/phases/04_dependencies.rs`
- [ ] Delete `src/pipeline/phases/05_build_order.rs`
- [ ] Update `src/pipeline/orchestrator.rs`:
  - [ ] Remove `DependenciesPhase` and `BuildOrderPhase` from workflow
  - [ ] Final phase list: Scan → WorkspaceStructure → RootCache → ServiceAnalysis → Assemble
- [ ] Update `src/pipeline/context.rs`:
  - [ ] Remove `dependencies: Option<DependencyResult>` field
  - [ ] Remove `build_order: Option<BuildOrderResult>` field
- [ ] Run all e2e tests and verify they pass
- [ ] Verify workflow phase count reduced from 6 to 5

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
