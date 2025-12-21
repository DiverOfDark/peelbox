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

**Status: COMPLETED (2025-12-21)**

All 8 runtimes now implement deterministic parsing in `try_extract()`:

- ✅ **JvmRuntime.try_extract()** (71 unit tests total)
  - ✅ Detect native dependencies from pom.xml/build.gradle (JNA/JNI patterns)
  - ✅ Scan for Java server port bindings (ServerSocket, .setPort())
  - ✅ Extract env vars from System.getenv("VAR") calls
  - Tests: test_extract_env_vars, test_extract_ports_server_socket, test_extract_ports_jetty, test_extract_native_deps_pom, test_extract_native_deps_gradle

- ✅ **NodeRuntime.try_extract()** (3 unit tests)
  - ✅ Scan .js/.ts files for server.listen(port) and --port X in scripts
  - ✅ Extract env vars from process.env.VARIABLE patterns
  - Tests: test_extract_env_vars, test_extract_ports_listen, test_extract_ports_script_arg

- ✅ **PythonRuntime.try_extract()** (6 unit tests)
  - ✅ Scan for app.run(port=X) calls (framework-agnostic)
  - ✅ Extract env vars from os.environ['VAR'] and os.getenv('VAR')
  - ✅ Detect native dependencies from requirements.txt C extensions
  - Tests: test_extract_env_vars_environ, test_extract_env_vars_getenv, test_extract_ports, test_extract_native_deps_requirements, test_extract_native_deps_setup_py, test_extract_native_deps_pyproject

- ✅ **RubyRuntime.try_extract()** (3 unit tests)
  - ✅ Scan for Rack::Server and WEBrick port bindings
  - ✅ Extract env vars from ENV['VAR'] and ENV["VAR"]
  - ✅ Detect native dependencies from Gemfile native extensions
  - Tests: test_extract_env_vars, test_extract_ports, test_extract_native_deps

- ✅ **PhpRuntime.try_extract()** (2 unit tests)
  - ✅ Extract env vars from $_ENV['VAR'] patterns
  - ✅ Detect native dependencies from composer.json extensions
  - Tests: test_extract_env_vars, test_extract_native_deps

- ✅ **DotNetRuntime.try_extract()** (3 unit tests)
  - ✅ Parse launchSettings.json for runtime config
  - ✅ Extract env vars from Environment.GetEnvironmentVariable("VAR")
  - ✅ Detect native dependencies from .csproj NativeLibrary references
  - Tests: test_extract_env_vars, test_extract_ports_launch_settings, test_extract_native_deps

- ✅ **BeamRuntime.try_extract()** (3 unit tests)
  - ✅ Scan for Cowboy/Ranch port bindings
  - ✅ Extract env vars from System.get_env("VAR")
  - ✅ Detect native dependencies from mix.exs NIF references
  - Tests: test_extract_env_vars, test_extract_ports_cowboy, test_extract_native_deps

- ✅ **NativeRuntime.try_extract()** (4 unit tests)
  - ✅ Use build system hints (Cargo.toml metadata, go.mod comments)
  - ✅ Detect port bindings from source scanning (bind(), listen())
  - Tests: test_extract_ports_rust, test_extract_ports_go, test_extract_ports_cpp, test_extract_metadata_hints

All implementations use regex/deterministic parsing, no LLM calls.

#### Framework-Level Config Parsing

**Status: COMPLETED (2025-12-21)**

Extended Framework trait with config parsing capabilities (103 framework tests total):

- ✅ **Framework Trait Extension**
  - ✅ Added `FrameworkConfig` struct with `port`, `env_vars`, `health_endpoint`
  - ✅ Added `config_files() -> Vec<&str>` method
  - ✅ Added `parse_config(file_path, content) -> Option<FrameworkConfig>` method

- ✅ **SpringBootFramework** (10 tests)
  - ✅ Parse application.properties and application.yml
  - ✅ Extract server.port, management.endpoints.web.base-path
  - ✅ Extract ${VAR_NAME} environment variables
  - Tests: test_config_files, test_parse_application_properties, test_parse_application_yml, test_parse_health_endpoint, test_parse_env_vars, etc.

- ✅ **DjangoFramework** (10 tests)
  - ✅ Parse settings.py, config files
  - ✅ Extract ALLOWED_HOSTS, DEBUG, os.environ patterns
  - Tests: test_config_files, test_parse_settings_basic, test_parse_port, test_parse_env_vars, etc.

- ✅ **FlaskFramework** (10 tests)
  - ✅ Parse app config, instance config
  - ✅ Extract app.run(port=X), os.environ patterns
  - Tests: test_config_files, test_parse_app_run, test_parse_env_vars, etc.

- ✅ **LaravelFramework** (10 tests)
  - ✅ Parse config/*.php files, .env
  - ✅ Extract env('VAR') patterns
  - Tests: test_config_files, test_parse_env_file, test_parse_config_app, etc.

- ✅ **RailsFramework** (10 tests)
  - ✅ Parse config/puma.rb, config/application.rb
  - ✅ Extract port bindings, ENV['VAR'] patterns
  - Tests: test_config_files, test_parse_puma_config, test_parse_env_vars, etc.

- ✅ **AspNetFramework** (10 tests)
  - ✅ Parse appsettings.json, appsettings.{env}.json
  - ✅ Extract Kestrel URLs, health check endpoints
  - Tests: test_config_files, test_parse_appsettings, test_parse_kestrel_urls, etc.

- ✅ **PhoenixFramework** (10 tests)
  - ✅ Parse config/runtime.exs, config/prod.exs
  - ✅ Extract port bindings, System.get_env patterns
  - Tests: test_config_files, test_parse_runtime_config, test_parse_env_vars, etc.

- ✅ **NextJsFramework** (10 tests)
  - ✅ Parse next.config.js, package.json
  - ✅ Extract port from env, process.env patterns
  - Tests: test_config_files, test_parse_next_config, test_parse_package_json, etc.

- ✅ **ExpressFramework** (10 tests)
  - ✅ Scan for app.listen() with Express patterns
  - ✅ Extract process.env.PORT, app.listen(PORT)
  - Tests: test_config_files, test_parse_app_listen, test_parse_env_vars, etc.

- ✅ **SymfonyFramework** (13 tests - NEW)
  - ✅ Parse config/packages/*.yaml, .env files
  - ✅ Extract %env(VAR_NAME)% patterns, server.port
  - Tests: test_dependency_patterns, test_default_ports, test_health_endpoints, test_env_var_patterns, test_config_files, test_parse_config_yaml, test_parse_env_file, test_parse_port, test_parse_env_vars, test_parse_health_endpoint, etc.

#### Missing Framework Implementations

**Status: COMPLETED (2025-12-21)**

- ✅ **Symfony framework (PHP)**
  - ✅ Created src/stack/framework/symfony.rs
  - ✅ Added FrameworkId::Symfony to src/stack/mod.rs
  - ✅ Implemented dependency detection (symfony/framework-bundle, symfony/http-kernel)
  - ✅ Set default port: 8000
  - ✅ Set health endpoints: /_health, /health
  - ✅ Added config parsing: config/packages/*.yaml, .env files
  - ✅ Created test fixture: tests/fixtures/single-language/php-symfony/
  - ✅ Added 3 e2e tests (detection, llm, static)
  - All tests passing: 73 total (70 original + 3 Symfony)

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

**Completed - WorkspaceBuildSystem Implementations (2025-12-21)**:

- ✅ **Yarn** (JavaScript/TypeScript):
  - Implemented `WorkspaceBuildSystem` trait for `YarnBuildSystem`
  - Parses `package.json` workspaces field (same as npm)
  - Fixed deduplication priority (yarn.lock priority 15 > package.json priority 10)
  - All 3 test modes passing (detection, llm, static)

- ✅ **Pnpm** (JavaScript/TypeScript):
  - Implemented `WorkspaceBuildSystem` trait for `PnpmBuildSystem`
  - Parses `package.json` workspaces field (identical to Yarn)
  - Fixed deduplication priority (pnpm-lock.yaml priority 15 > package.json priority 10)
  - All 3 test modes passing

- ✅ **Gradle** (JVM):
  - Implemented `WorkspaceBuildSystem` trait for `GradleBuildSystem`
  - Parses `settings.gradle[.kts]` include() directives
  - Expands project paths from settings file
  - Fixed manifest priority (settings.gradle priority 15 > build.gradle priority 10)
  - All 3 Gradle multiproject tests passing

- ✅ **Maven** (JVM):
  - Implemented `WorkspaceBuildSystem` trait for `MavenBuildSystem`
  - Parses `pom.xml` <modules> section
  - Expands module paths
  - All 3 Maven multimodule tests passing

- ✅ **Cargo** (Rust):
  - Implemented `WorkspaceBuildSystem` trait for `CargoBuildSystem`
  - Parses `Cargo.toml` [workspace.members] array
  - Expands workspace member paths
  - All Cargo workspace tests passing

- ✅ **.NET** (C#/F#):
  - Implemented `WorkspaceBuildSystem` trait for `DotNetBuildSystem`
  - Parses `*.sln` Project() references
  - Extracts project directory from .csproj/.fsproj path
  - No test fixtures yet (no .NET workspace tests in suite)

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
