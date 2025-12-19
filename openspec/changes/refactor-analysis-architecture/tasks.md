# Tasks: Analysis Architecture Refactoring

## Track 1: Service Phases (PRs 1-8)

### PR1: Runtime Trait Infrastructure (~200 LOC)

- [ ] Create `src/runtime/mod.rs` module
- [ ] Define `Runtime` trait with methods:
  - [ ] `try_deterministic_config(files, framework) -> Option<RuntimeConfig>`
  - [ ] `extract_config_llm(files, framework) -> Result<RuntimeConfig>`
  - [ ] `runtime_base_image(version) -> String`
  - [ ] `required_packages() -> Vec<&str>`
  - [ ] `start_command(entrypoint) -> String`
- [ ] Define `RuntimeConfig` struct with fields: `entrypoint`, `port`, `env_vars`, `health`, `native_deps`
- [ ] Define `HealthCheck` struct with field: `endpoint`
- [ ] Create `src/runtime/jvm.rs`
- [ ] Implement `JvmRuntime` with all trait methods
- [ ] Add unit tests for `JvmRuntime`
- [ ] Verify trait compiles and tests pass

### PR2: Complete Runtime Implementations (~300 LOC)

**Depends on**: PR1

- [ ] Create `src/runtime/node.rs`
- [ ] Implement `NodeRuntime` with all trait methods
- [ ] Add unit tests for `NodeRuntime`
- [ ] Create `src/runtime/python.rs`
- [ ] Implement `PythonRuntime` with all trait methods
- [ ] Add unit tests for `PythonRuntime`
- [ ] Create `src/runtime/ruby.rs`
- [ ] Implement `RubyRuntime` with all trait methods
- [ ] Add unit tests for `RubyRuntime`
- [ ] Create `src/runtime/php.rs`
- [ ] Implement `PhpRuntime` with all trait methods
- [ ] Add unit tests for `PhpRuntime`
- [ ] Create `src/runtime/dotnet.rs`
- [ ] Implement `DotNetRuntime` with all trait methods
- [ ] Add unit tests for `DotNetRuntime`
- [ ] Create `src/runtime/beam.rs`
- [ ] Implement `BeamRuntime` with all trait methods
- [ ] Add unit tests for `BeamRuntime`
- [ ] Create `src/runtime/native.rs`
- [ ] Implement `NativeRuntime` with all trait methods
- [ ] Add unit tests for `NativeRuntime`
- [ ] Create `src/runtime/llm.rs`
- [ ] Implement `LLMRuntime` (fallback) with all trait methods
- [ ] Add unit tests for `LLMRuntime`
- [ ] Verify all runtime implementations compile and tests pass

### PR3: Add Health Endpoint to Schema (~100 LOC)

**Independent** (can run in parallel with PR1-2)

- [ ] Add `HealthCheck` struct to `src/output/schema.rs`
- [ ] Add `health: Option<HealthCheck>` field to `RuntimeStage`
- [ ] Update schema validation for health field
- [ ] Update all test fixtures in `tests/fixtures/expected/` to include `"health": null`
- [ ] Run schema validation tests
- [ ] Verify all e2e tests still pass

### PR4: RuntimeConfigPhase Integration (~200 LOC)

**Depends on**: PR1, PR2

- [ ] Create `src/pipeline/phases/07_runtime_config.rs`
- [ ] Implement `RuntimeConfigPhase` using `Runtime.extract_config()`
- [ ] Add deterministic-first pattern:
  - [ ] Try `runtime.try_deterministic_config(files, framework)`
  - [ ] Fallback to `runtime.extract_config_llm(files, framework)`
- [ ] Update `src/pipeline/phases/07_service_analysis.rs`:
  - [ ] Add `RuntimeConfigPhase` to phase list
  - [ ] Comment out old phases: `EntrypointPhase`, `PortPhase`, `EnvVarsPhase`, `HealthPhase`, `NativeDepsPhase`
- [ ] Update `src/pipeline/service_context.rs`:
  - [ ] Add `runtime_config: Option<RuntimeConfig>` field
- [ ] Wire runtime detection from `RuntimePhase` to `RuntimeConfigPhase`
- [ ] Run all e2e tests and verify they pass
- [ ] Verify same output as before (different implementation)

### PR5: Remove Old Service Phases (~100 LOC - deletions)

**Depends on**: PR4

- [ ] Delete `src/pipeline/phases/07_3_entrypoint.rs`
- [ ] Delete `src/pipeline/phases/07_5_port.rs`
- [ ] Delete `src/pipeline/phases/07_6_env_vars.rs`
- [ ] Delete `src/pipeline/phases/07_7_health.rs`
- [ ] Delete `src/pipeline/phases/07_4_native_deps.rs`
- [ ] Clean up `src/pipeline/phases/07_service_analysis.rs`:
  - [ ] Remove commented phase references
  - [ ] Clean up phase list to only include active phases
- [ ] Update `src/pipeline/service_context.rs`:
  - [ ] Remove old fields (individual port, env_vars, health, native_deps)
  - [ ] Keep only `runtime_config: RuntimeConfig`
- [ ] Run all e2e tests and verify they still pass
- [ ] Verify service phase count reduced from 8 to 4

### PR6: Use Framework Defaults in Runtime (~150 LOC)

**Depends on**: PR4

- [ ] Update `src/runtime/jvm.rs` to use framework defaults:
  - [ ] Use `framework.default_ports()` as fallback for port
  - [ ] Use `framework.health_endpoints()` for health check
- [ ] Update `src/runtime/node.rs` with framework defaults
- [ ] Update `src/runtime/python.rs` with framework defaults
- [ ] Update `src/runtime/ruby.rs` with framework defaults
- [ ] Update `src/runtime/php.rs` with framework defaults
- [ ] Update `src/runtime/dotnet.rs` with framework defaults
- [ ] Update `src/runtime/beam.rs` with framework defaults
- [ ] Update `src/runtime/native.rs` with framework defaults
- [ ] Update `src/pipeline/phases/07_runtime_config.rs`:
  - [ ] Pass detected framework to `runtime.extract_config()`
- [ ] Add integration tests for framework-specific defaults
- [ ] Run all e2e tests and verify they pass
- [ ] Verify Spring Boot apps detect port 8080 and `/actuator/health`
- [ ] Verify Next.js apps detect port 3000

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
- [ ] Implement deterministic config parsing in JvmRuntime.try_deterministic_config()
  - [ ] Parse application.properties for port and configuration
  - [ ] Parse application.yml for port and configuration
  - [ ] Extract env vars from @Value annotations or Environment usage
  - [ ] Detect native dependencies from pom.xml/build.gradle
- [ ] Implement LLM-based config extraction in JvmRuntime.extract_config_llm()
  - [ ] Design minimal LLM prompt for config extraction
  - [ ] Call LLM client with file context
  - [ ] Parse and validate LLM response
- [ ] Implement deterministic/LLM config extraction for all other runtimes
  - [ ] NodeRuntime: Parse package.json scripts, scan for app.listen() calls
  - [ ] PythonRuntime: Parse Flask/Django config files, scan for app.run()
  - [ ] RubyRuntime: Parse config/puma.rb, Rack config
  - [ ] PhpRuntime: Parse php.ini, framework config files
  - [ ] DotNetRuntime: Parse appsettings.json, launchSettings.json
  - [ ] BeamRuntime: Parse config/runtime.exs, config/prod.exs
  - [ ] NativeRuntime: Determine strategy (binary inspection vs manifest hints)
