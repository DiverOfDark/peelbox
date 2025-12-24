# Spec Delta: Stack Registry with Custom Types

## ADDED Requirements

### Requirement: Custom Type Support

The stack registry SHALL support dynamically-discovered types from LLM analysis in addition to hardcoded enum variants.

#### Scenario: Known type uses enum variant

Given a Rust project with Cargo.toml
When the registry detects the build system
Then it returns `BuildSystemId::Cargo` (known variant)
And no LLM call is made

#### Scenario: Unknown type uses Custom variant

Given a project with Bazel BUILD file
When the registry detects the build system
And pattern-based detection fails
Then it calls LLM to identify the build system
And returns `BuildSystemId::Custom("Bazel")`

#### Scenario: Custom type implements trait

Given a `CustomBuildSystem` created from LLM response
When the system calls trait methods
Then it returns LLM-provided metadata (manifest files, build commands, cache dirs)

### Requirement: LLM Fallback Detection

Registry SHALL attempt pattern-based detection before falling back to LLM.

#### Scenario: Pattern match succeeds (fast path)

Given a Node.js project with package.json
When detecting build system
Then pattern-based detection succeeds immediately
And LLM is not called
And response time < 5ms

#### Scenario: Pattern match fails (slow path)

Given a project with unknown manifest format
When detecting build system
Then pattern-based detection returns None
And LLM identification is attempted
And response time ~200-500ms

#### Scenario: LLM identifies known type

Given a Gradle project with unusual file structure
When pattern detection fails
And LLM identifies it as "Gradle"
Then registry returns `BuildSystemId::Gradle` (known variant)
And does not create custom type

### Requirement: Runtime Registration

Registry SHALL support dynamic registration of LLM-discovered types.

#### Scenario: Register custom language

Given LLM identifies language as "Zig"
When `register_language_runtime()` is called with `CustomLanguage`
Then language is stored in registry
And `get_language(LanguageId::Custom("Zig"))` returns the implementation

#### Scenario: Prevent known type registration via runtime method

Given a caller attempts `register_language_runtime(LanguageId::Rust, ...)`
When the method is called
Then it panics with error message
And suggests using `register_language()` instead

### Requirement: Type Enumeration

All ID enums SHALL support custom variants for LLM-discovered types.

#### Scenario: LanguageId serialization

Given `LanguageId::Rust`
When serialized to JSON
Then output is `"rust"` (lowercase)

Given `LanguageId::Custom("Zig")`
When serialized to JSON
Then output is `"zig"` (custom name as-is)

#### Scenario: BuildSystemId deserialization

Given JSON `"cargo"`
When deserialized
Then result is `BuildSystemId::Cargo`

Given JSON `"bazel"`
When deserialized
And "bazel" is not a known variant
Then result is `BuildSystemId::Custom("bazel")`

#### Scenario: Pattern matching exhaustiveness

Given a match expression on LanguageId
When compiling
Then Rust compiler enforces handling `Custom` variant
And non-exhaustive matches fail compilation

## REMOVED Requirements

None - this is purely additive functionality.
