# prompt-pipeline Spec Deltas

## MODIFIED Requirements

### Requirement: Deterministic Parsing

The system SHALL use deterministic parsers for known manifest formats, bypassing LLM calls when possible.

**Implementation Note**: Parsers are consolidated in `src/languages/parsers.rs` as reusable traits to eliminate duplication:
- `TomlDependencyParser` - Shared by Rust (Cargo.toml), Python (pyproject.toml with Poetry)
- `JsonDependencyParser` - Shared by Node.js (package.json), npm/yarn/pnpm monorepos
- `RegexDependencyParser` - Generic regex-based parser for simple dependency formats

Build system detection now uses `BuildSystemRegistry` which delegates to individual build system implementations. Each build system (Maven, Gradle, npm, etc.) implements the `BuildSystem` trait with its own `detect()` and `parse_dependencies()` methods.

#### Scenario: Node.js package.json parsing

**Given** a repository with a valid `package.json`
**When** phase 4 executes
**Then** it SHALL extract dependencies using `NodeParser`
**And** it SHALL NOT make an LLM call
**And** the result SHALL indicate `detected_by: DetectionMethod::Deterministic`
**And** confidence SHALL be `high`

#### Scenario: Rust Cargo.toml parsing

**Given** a Cargo workspace with multiple members
**When** phase 4 executes
**Then** it SHALL parse `Cargo.toml` to extract workspace members
**And** it SHALL identify internal dependencies between members
**And** it SHALL NOT make an LLM call

#### Scenario: Supported manifest formats

**Given** the system is analyzing a repository
**Then** it SHALL support deterministic parsing for:
- `package.json` (Node.js)
- `pnpm-workspace.yaml` (pnpm monorepos)
- `Cargo.toml` (Rust)
- `go.mod` (Go)
- `pom.xml` (Maven)
- `build.gradle`, `build.gradle.kts` (Gradle)
- `pyproject.toml`, `requirements.txt` (Python)

---

### Requirement: Code-Based Extraction

The system SHALL extract structured data from code and configuration files before invoking LLM prompts, reducing context size.

**Implementation Note**: Extractors now use shared scanning logic from `src/extractors/common.rs` to eliminate duplication. The `scan_directory_with_patterns<F>()` function provides a unified scanning implementation used by port, environment variable, and health check extractors.

#### Scenario: Port extraction from code

**Given** a Node.js service with `app.listen(3000)`
**When** phase 6e (port discovery) executes
**Then** the port extractor SHALL find port 3000 via regex
**And** the LLM prompt SHALL include the extracted snippet, not full file

#### Scenario: Environment variable extraction

**Given** a service with `.env.example` containing `DATABASE_URL=`
**When** phase 6f (env vars discovery) executes
**Then** the env vars extractor SHALL find `DATABASE_URL`
**And** the LLM prompt SHALL include the extracted variable names, not full file

#### Scenario: Health check extraction from routes

**Given** a Spring Boot service with `@GetMapping("/actuator/health")`
**When** phase 6g (health check discovery) executes
**Then** the health check extractor SHALL find the route via regex
**And** the LLM prompt SHALL include the matched route definition

---

## ADDED Requirements

### Requirement: Build System Abstraction

The system SHALL treat build systems as first-class entities separate from language definitions, enabling reusability across languages.

**Implementation Details**:
- Build systems implement the `BuildSystem` trait defined in `src/build_systems/mod.rs`
- Each build system knows its manifest patterns, cache paths, and dependency parsing logic
- `BuildSystemRegistry` provides centralized lookup and detection
- Languages declare compatibility via `compatible_build_systems()` method

**Supported Build Systems** (13 total):
1. **Maven** (`pom.xml`) - Java, Kotlin, Scala, Groovy
2. **Gradle** (`build.gradle`, `build.gradle.kts`) - Java, Kotlin, Scala, Groovy
3. **npm** (`package.json` + `package-lock.json`) - JavaScript, TypeScript
4. **yarn** (`package.json` + `yarn.lock`) - JavaScript, TypeScript
5. **pnpm** (`package.json` + `pnpm-lock.yaml`) - JavaScript, TypeScript
6. **bun** (`package.json` + `bun.lockb`) - JavaScript, TypeScript
7. **pip** (`requirements.txt`) - Python
8. **poetry** (`pyproject.toml` with `[tool.poetry]`) - Python
9. **pipenv** (`Pipfile`) - Python
10. **cargo** (`Cargo.toml`) - Rust
11. **go** (`go.mod`) - Go
12. **dotnet** (`*.csproj`, `*.fsproj`) - C#, F#
13. **composer** (`composer.json`) - PHP

#### Scenario: Build system reusability

**Given** Java and Kotlin projects both use Maven
**When** build templates are generated
**Then** both SHALL use the same `MavenBuildSystem` implementation
**And** Maven logic SHALL be defined only once in `src/build_systems/maven.rs`

#### Scenario: Many-to-many relationships

**Given** a language supports multiple build systems
**When** querying compatible build systems
**Then** the language SHALL declare all compatible systems via `compatible_build_systems()`
**And** the detection pipeline SHALL use `BuildSystemRegistry` to select the correct one

#### Scenario: Build system detection

**Given** a repository with a `pom.xml` file
**When** bootstrap scanner detects the manifest
**Then** `BuildSystemRegistry.detect()` SHALL return the Maven build system
**And** confidence SHALL be based on content analysis (presence of `<project>` tag)

---

### Requirement: Confidence Type Consolidation

The system SHALL use a single shared `Confidence` enum across all pipeline phases to eliminate duplication.

**Implementation Details**:
- `Confidence` enum defined in `src/pipeline/confidence.rs`
- All 11 pipeline phase files import from shared module
- Eliminates previous 11× duplication across phase files

#### Scenario: Shared confidence type

**Given** any pipeline phase executes
**When** a result is produced with a confidence score
**Then** it SHALL use `crate::pipeline::confidence::Confidence`
**And** the confidence SHALL be one of: `High`, `Medium`, or `Low`

#### Scenario: Confidence serialization

**Given** a `UniversalBuild` is serialized to JSON
**When** the confidence field is written
**Then** it SHALL serialize as lowercase strings: `"high"`, `"medium"`, or `"low"`
**And** deserialization SHALL accept the same format

---

## REMOVED Requirements

None - This refactoring is internal optimization. All external behavior and requirements remain unchanged.
