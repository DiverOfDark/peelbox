## ADDED Requirements

### Requirement: Runtime Trait

The system SHALL provide a `Runtime` trait representing platform execution environments (JVM, Node, Python, Ruby, PHP, Native) with deterministic and LLM-based configuration extraction.

#### Scenario: Runtime configuration extraction with deterministic-first pattern
- **WHEN** a Runtime implementation extracts configuration
- **THEN** it first attempts `try_deterministic_config(files, framework)` to parse known formats
- **AND** falls back to `extract_config_llm(files, framework)` only if deterministic fails
- **AND** returns `RuntimeConfig` with entrypoint, port, env_vars, health, and native_deps
- **AND** uses framework defaults where available (port, health endpoint)

#### Scenario: Runtime base image with version parameter
- **WHEN** calling `runtime.runtime_base_image(version)`
- **THEN** NodeRuntime returns `node:{version}-alpine` (e.g., `node:20-alpine` for version "20")
- **AND** JvmRuntime returns `eclipse-temurin:{version}-jre-alpine`
- **AND** version defaults to latest stable if None provided

#### Scenario: Start command generation
- **WHEN** calling `runtime.start_command(entrypoint)`
- **THEN** JvmRuntime generates `java -jar {entrypoint}`
- **AND** NodeRuntime generates `node {entrypoint}`
- **AND** NativeRuntime generates `./{entrypoint}` for static binaries

#### Scenario: Required system packages
- **WHEN** calling `runtime.required_packages()`
- **THEN** JvmRuntime returns `["ca-certificates"]`
- **AND** NodeRuntime returns `["dumb-init"]`
- **AND** NativeRuntime returns empty Vec for static binaries

### Requirement: Runtime Implementations

The system SHALL provide Runtime trait implementations for all supported platform runtimes: JvmRuntime, NodeRuntime, PythonRuntime, RubyRuntime, PhpRuntime, DotNetRuntime, BeamRuntime, NativeRuntime, and LLMRuntime.

#### Scenario: JvmRuntime for Java/Kotlin/Scala
- **WHEN** using JvmRuntime for a Spring Boot application
- **THEN** deterministic config extraction parses `application.properties` or `application.yml` for port
- **AND** uses framework default port 8080 if not found
- **AND** uses framework health endpoint `/actuator/health` if Spring Boot detected
- **AND** identifies native dependencies from `pom.xml`/`build.gradle` (e.g., database drivers)

#### Scenario: NodeRuntime for JavaScript/TypeScript
- **WHEN** using NodeRuntime for an Express application
- **THEN** deterministic config extraction scans for `app.listen()` or `server.listen()` calls
- **AND** uses framework default port 3000 for Express/Next.js
- **AND** identifies env vars from `process.env.VARIABLE_NAME` patterns

#### Scenario: DotNetRuntime for C#/F#
- **WHEN** using DotNetRuntime for an ASP.NET Core application
- **THEN** deterministic config extraction parses `appsettings.json` for port and configuration
- **AND** runtime base image is `mcr.microsoft.com/dotnet/aspnet:{version}` (e.g., `mcr.microsoft.com/dotnet/aspnet:8.0`)
- **AND** start_command is `dotnet {assembly}.dll`
- **AND** identifies env vars from `IConfiguration` usage patterns

#### Scenario: BeamRuntime for Elixir
- **WHEN** using BeamRuntime for a Phoenix application
- **THEN** deterministic config extraction parses `config/runtime.exs` or `config/prod.exs` for port
- **AND** runtime base image is `hexpm/elixir:{version}-erlang-{otp_version}-alpine`
- **AND** start_command is `_build/prod/rel/{app_name}/bin/{app_name} start`
- **AND** identifies env vars from `System.get_env()` calls

#### Scenario: NativeRuntime for Rust/Go/C++
- **WHEN** using NativeRuntime for a Rust application
- **THEN** runtime base image is `alpine:latest` or `scratch` for static binaries
- **AND** required_packages is empty (binary includes all dependencies)
- **AND** start_command is `./binary_name` (no runtime interpreter needed)

#### Scenario: LLMRuntime fallback for unknown platforms
- **WHEN** using LLMRuntime for an unsupported language/framework
- **THEN** all methods fall back to LLM calls
- **AND** try_deterministic_config returns None
- **AND** extract_config_llm makes LLM call with minimal context

### Requirement: MonorepoOrchestrator Trait Extension

The system SHALL extend the `MonorepoOrchestrator` trait with workspace structure parsing, build order calculation, and workspace-aware build command generation.

#### Scenario: Workspace structure parsing
- **WHEN** calling `orchestrator.workspace_structure(repo_path)` on TurborepoOrchestrator
- **THEN** it parses `turbo.json` for workspace configuration
- **AND** identifies applications (packages with `turbo.json` tasks or scripts)
- **AND** identifies libraries (packages without runnable tasks)
- **AND** parses dependency graph from package.json workspaces
- **AND** returns `WorkspaceStructure` with all information

#### Scenario: Build order calculation
- **WHEN** TurborepoOrchestrator calculates build order
- **THEN** it performs topological sort on dependency graph
- **AND** ensures libraries are built before dependent applications
- **AND** returns ordered Vec<PathBuf> of packages to build

#### Scenario: Workspace-aware build commands
- **WHEN** calling `orchestrator.build_command(package, workspace)`
- **THEN** TurborepoOrchestrator returns `turbo run build --filter={package}`
- **AND** NxOrchestrator returns `nx build {package}`
- **AND** LernaOrchestrator returns `lerna run build --scope={package}`
- **AND** commands are scoped to specific package, not entire workspace

### Requirement: Version Detection

The system SHALL provide version detection via Language trait to enable version-aware runtime package selection.

#### Scenario: Node.js version from package.json
- **WHEN** JavaScriptLanguage detects version from `package.json`
- **THEN** it parses `engines.node` field (e.g., `">=20.0.0"`)
- **AND** extracts major version "20"
- **AND** returns `Some("20")` for use in runtime selection

#### Scenario: Java version from Maven POM
- **WHEN** JavaLanguage detects version from `pom.xml`
- **THEN** it parses `<java.version>21</java.version>`
- **AND** returns `Some("21")` for JVM base image selection

#### Scenario: No version specified
- **WHEN** Language cannot find version in manifest
- **THEN** it returns `None`
- **AND** Runtime uses default latest stable version

### Requirement: RuntimeConfig Struct

The system SHALL provide a `RuntimeConfig` struct that aggregates all runtime configuration properties extracted by Runtime trait.

#### Scenario: Complete runtime configuration
- **WHEN** RuntimeConfigPhase completes extraction
- **THEN** RuntimeConfig contains `entrypoint: Option<String>` (e.g., `Some("app.jar")`)
- **AND** contains `port: Option<u16>` (e.g., `Some(8080)`)
- **AND** contains `env_vars: Vec<String>` (e.g., `["DATABASE_URL", "API_KEY"]`)
- **AND** contains `health: Option<HealthCheck>` (e.g., endpoint, interval, timeout, retries)
- **AND** contains `native_deps: Vec<String>` (e.g., `["postgresql-libs", "curl"]`)

#### Scenario: HealthCheck struct
- **WHEN** RuntimeConfig includes health check
- **THEN** HealthCheck contains `endpoint: String` (e.g., `/actuator/health`)
- **AND** contains `interval: Option<String>` (e.g., `Some("30s")`)
- **AND** contains `timeout: Option<String>` (e.g., `Some("5s")`)
- **AND** contains `retries: Option<u32>` (e.g., `Some(3)`)
