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

