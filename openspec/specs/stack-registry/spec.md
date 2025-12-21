# stack-registry Specification

## Purpose
TBD - created by archiving change unify-registry-chain. Update Purpose after archive.
## Requirements
### Requirement: Type-Safe Stack Identifiers

The system SHALL use strongly-typed enums for all technology stack identifiers instead of strings.

#### Scenario: Language identification with typed enum

**Given** a Rust language implementation
**When** calling `language.id()`
**Then** it SHALL return `LanguageId::Rust` (not a string)
**And** the result SHALL be comparable with `==` without string matching
**And** typos in identifiers SHALL cause compile errors, not runtime bugs

#### Scenario: Build system identification with typed enum

**Given** a Maven build system implementation
**When** calling `build_system.id()`
**Then** it SHALL return `BuildSystemId::Maven`
**And** it SHALL be usable as a HashMap key (Hash + Eq)

#### Scenario: Framework identification with typed enum

**Given** a Spring Boot framework implementation
**When** calling `framework.id()`
**Then** it SHALL return `FrameworkId::SpringBoot`
**And** it SHALL serialize to JSON as "spring-boot" via serde

#### Scenario: Invalid identifier compilation failure

**Given** code attempting to use an invalid identifier
**When** compiling `LanguageId::InvalidLang`
**Then** compilation SHALL fail with error
**And** there SHALL be no runtime string matching errors

---

### Requirement: Typed Compatibility Declarations

Framework and language implementations SHALL declare compatibility using typed enum arrays, not string arrays.

#### Scenario: Framework declares compatible languages with types

**Given** Spring Boot framework implementation
**When** calling `framework.compatible_languages()`
**Then** it SHALL return `&[LanguageId::Java, LanguageId::Kotlin]`
**And** NOT return `&["Java", "Kotlin"]`

#### Scenario: Language declares compatible build systems with types

**Given** Rust language implementation
**When** calling `language.compatible_build_systems()`
**Then** it SHALL return `&[BuildSystemId::Cargo]`
**And** the compiler SHALL enforce only valid BuildSystemId variants

#### Scenario: Compile-time validation of compatibility arrays

**Given** a framework implementation with a typo in compatible_languages
**When** compiling `&[LanguageId::Jav]` (typo)
**Then** compilation SHALL fail
**And** NOT produce a runtime error with string matching

---

### Requirement: Unified Stack Registry

The system SHALL provide a single StackRegistry that combines BuildSystem, Language, and Framework registries with pre-computed relationship maps.

#### Scenario: Single registry replaces three

**Given** pipeline initialization
**When** creating PipelineOrchestrator
**Then** it SHALL use one StackRegistry
**And** NOT require LanguageRegistry, BuildSystemRegistry, and FrameworkRegistry separately

#### Scenario: Relationship maps are pre-computed

**Given** StackRegistry with defaults loaded
**When** calling `stack_registry.get_compatible_languages(BuildSystemId::Maven)`
**Then** it SHALL return `&[LanguageId::Java, LanguageId::Kotlin]` in O(1) time
**And** NOT iterate through all languages

#### Scenario: Bi-directional relationship lookup

**Given** StackRegistry with all entities registered
**When** querying `get_compatible_frameworks(LanguageId::Java)`
**Then** it SHALL return all frameworks compatible with Java
**And** when querying `get_compatible_languages(FrameworkId::SpringBoot)`
**Then** it SHALL return all languages compatible with Spring Boot

---

### Requirement: Automatic Chain Detection

The system SHALL automatically detect the full technology stack (BuildSystem → Language → Framework) from a manifest file.

#### Scenario: Detect full stack from package.json

**Given** a repository with `package.json` containing `"express": "^4.18.0"`
**When** calling `stack_registry.detect_stack("package.json", content)`
**Then** it SHALL return `DetectionStack { build_system: Npm, language: JavaScript, framework: Some(Express) }`
**And** NOT require separate calls to three registries

#### Scenario: Detect stack without framework

**Given** a repository with `Cargo.toml` without framework dependencies
**When** calling `stack_registry.detect_stack("Cargo.toml", content)`
**Then** it SHALL return `DetectionStack { build_system: Cargo, language: Rust, framework: None }`

#### Scenario: Chain detection rejects invalid combinations

**Given** a corrupted manifest with mixed signals (impossible combination)
**When** detection produces BuildSystem + Language that are incompatible
**Then** `detect_stack()` SHALL return None
**And** NOT return an invalid DetectionStack

---

### Requirement: Stack Validation

The system SHALL validate that BuildSystem-Language-Framework combinations are valid according to declared compatibility.

#### Scenario: Validate valid stack combination

**Given** StackRegistry with all defaults
**When** calling `validate_stack(BuildSystemId::Maven, LanguageId::Java, Some(FrameworkId::SpringBoot))`
**Then** it SHALL return true

#### Scenario: Reject invalid language for build system

**Given** StackRegistry with all defaults
**When** calling `validate_stack(BuildSystemId::Cargo, LanguageId::Python, None)`
**Then** it SHALL return false
**And** the reason SHALL be that Cargo is not compatible with Python

#### Scenario: Reject invalid framework for language

**Given** StackRegistry with all defaults
**When** calling `validate_stack(BuildSystemId::Npm, LanguageId::JavaScript, Some(FrameworkId::SpringBoot))`
**Then** it SHALL return false
**And** the reason SHALL be that Spring Boot is not compatible with JavaScript

#### Scenario: Validate all registered combinations

**Given** StackRegistry::with_defaults() loaded
**When** calling `validate_all_relationships()`
**Then** it SHALL return empty Vec (no errors)
**And** all registered entities SHALL have valid compatibility declarations

---

### Requirement: Framework Detection from Dependencies

The system SHALL detect frameworks deterministically from dependency lists using the unified registry.

#### Scenario: Detect Spring Boot from Maven dependencies

**Given** a DependencyInfo with `org.springframework.boot:spring-boot-starter-web`
**And** language is LanguageId::Java
**When** calling `stack_registry.detect_framework_from_deps(LanguageId::Java, &deps)`
**Then** it SHALL return `Some(FrameworkId::SpringBoot)`

#### Scenario: Detect Express from npm dependencies

**Given** a DependencyInfo with `express` package
**And** language is LanguageId::JavaScript
**When** calling `stack_registry.detect_framework_from_deps(LanguageId::JavaScript, &deps)`
**Then** it SHALL return `Some(FrameworkId::Express)`

#### Scenario: Filter incompatible frameworks by language

**Given** a DependencyInfo with `spring-boot-starter-web` dependency
**And** language is LanguageId::Python (invalid)
**When** calling `stack_registry.detect_framework_from_deps(LanguageId::Python, &deps)`
**Then** it SHALL return None
**And** NOT detect Spring Boot (because it's incompatible with Python)

---

### Requirement: Multi-Language Build System Handling

The system SHALL detect the primary language in multi-language build systems by counting files.

#### Scenario: Detect primary language in Kotlin/Java Gradle project

**Given** a Gradle project with 150 .kt files and 20 .java files
**When** calling `stack_registry.detect_primary_language(BuildSystemId::Gradle, &file_counts)`
**Then** it SHALL return `Some(LanguageId::Kotlin)`
**And** NOT return Java (fewer files)

#### Scenario: Detect primary language in TypeScript/JavaScript npm project

**Given** an npm project with 80 .ts files and 5 .js files
**When** calling `stack_registry.detect_primary_language(BuildSystemId::Npm, &file_counts)`
**Then** it SHALL return `Some(LanguageId::TypeScript)`

#### Scenario: No compatible language files found

**Given** a Cargo project with file_counts containing only JavaScript files
**When** calling `stack_registry.detect_primary_language(BuildSystemId::Cargo, &file_counts)`
**Then** it SHALL return None
**And** NOT return JavaScript (incompatible with Cargo)

#### Scenario: Tie-breaking with equal file counts

**Given** a Gradle project with exactly 50 .kt files and 50 .java files
**When** calling `stack_registry.detect_primary_language(BuildSystemId::Gradle, &file_counts)`
**Then** it SHALL return one of the compatible languages consistently
**And** use deterministic tie-breaking (e.g., lexicographic order)

---

### Requirement: Compile-Time Exhaustiveness

The system SHALL use Rust's exhaustive pattern matching to ensure all enum variants are handled.

#### Scenario: Match expression requires all languages

**Given** code with `match language_id { LanguageId::Rust => ..., LanguageId::Java => ..., }`
**When** compiling
**Then** compiler SHALL error if any LanguageId variant is missing
**And** force developer to handle all cases

#### Scenario: New enum variant breaks compilation

**Given** a new `LanguageId::Swift` variant added
**When** compiling existing match expressions
**Then** all incomplete matches SHALL fail compilation
**And** force updates to handle Swift

---

### Requirement: Serialization Compatibility

Typed enums SHALL serialize to human-readable strings for JSON output and configuration files.

#### Scenario: LanguageId serializes to lowercase

**Given** `LanguageId::JavaScript`
**When** serializing to JSON
**Then** it SHALL produce `"javascript"`

#### Scenario: BuildSystemId with custom serialization

**Given** `BuildSystemId::DotNet`
**When** serializing to JSON
**Then** it SHALL produce `"dotnet"` (not "DotNet")

#### Scenario: FrameworkId with kebab-case

**Given** `FrameworkId::SpringBoot`
**When** serializing to JSON
**Then** it SHALL produce `"spring-boot"`

#### Scenario: Deserialization from legacy strings

**Given** JSON containing `{"language": "javascript"}`
**When** deserializing to LanguageId
**Then** it SHALL parse as `LanguageId::JavaScript`
**And** be case-insensitive where appropriate

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

