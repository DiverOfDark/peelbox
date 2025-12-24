# llm-identification Specification

## Purpose
TBD - created by archiving change add-llm-dynamic-types. Update Purpose after archive.
## Requirements
### Requirement: Build System Identification

LLM client SHALL support identifying unknown build systems from manifest analysis.

#### Scenario: Identify Bazel build system

Given a BUILD file with content
```
load("@rules_rust//rust:defs.bzl", "rust_binary")

rust_binary(
    name = "hello",
    srcs = ["main.rs"],
)
```
When `llm.identify_build_system(path, content)` is called
Then response contains:
- `name: "Bazel"`
- `manifest_files: ["BUILD", "WORKSPACE"]`
- `build_commands: ["bazel build //..."]`
- `cache_dirs: ["bazel-out"]`
- `confidence: > 0.8`

#### Scenario: Identify unknown manifest format

Given a file with no recognizable structure
When `identify_build_system()` is called
Then response has `confidence < 0.5`
And system should not register custom type (low confidence)

### Requirement: Language Identification

LLM SHALL identify programming languages from manifest and context.

#### Scenario: Identify Zig from build.zig

Given a build.zig file
And file extensions: [".zig"]
When `llm.identify_language(path, content, build_system)` is called
Then response contains:
- `name: "Zig"`
- `file_extensions: [".zig"]`
- `package_managers: ["zig"]`
- `confidence: > 0.85`
- `reasoning: "Detected build.zig manifest and .zig source files"`

#### Scenario: Pattern detection prevents LLM custom type

Given package.json with TypeScript dependencies
When pattern detection identifies TypeScript
Then system returns `LanguageId::TypeScript` (known variant)
And LLM is never called
And no custom type is created

### Requirement: Framework Identification

LLM SHALL identify web/application frameworks from dependencies.

#### Scenario: Identify Fresh framework

Given dependencies: `["@fresh/core", "@preact/signals"]`
And language: "TypeScript"
When `llm.identify_framework(deps, lang)` is called
Then response contains:
- `name: "Fresh"`
- `language: "TypeScript"`
- `dependency_patterns: ["@fresh/*"]`
- `confidence: > 0.9`

#### Scenario: Multiple frameworks detected

Given dependencies for both Express and Apollo
When `identify_framework()` is called
Then response contains array with multiple frameworks
And each has unique `name` and `confidence`

### Requirement: Orchestrator Identification

LLM SHALL identify monorepo orchestrators from configuration files.

#### Scenario: Identify Moon orchestrator

Given moon.yml config file with workspace definition
When `llm.identify_orchestrator(config_files)` is called
Then response contains:
- `name: "Moon"`
- `config_files: ["moon.yml", ".moon/workspace.yml"]`
- `cache_dirs: [".moon/cache"]`

#### Scenario: No orchestrator present

Given standard single-project structure
When `identify_orchestrator()` is called
Then response is `None`
And no custom orchestrator is registered

### Requirement: Response Validation

LLM responses SHALL be validated before creating custom types.

#### Scenario: Confidence threshold

Given LLM response with `confidence: 0.3`
When validating response
Then system rejects the identification
And returns error indicating low confidence

#### Scenario: Required fields present

Given LLM response missing `name` field
When parsing response
Then system returns error
And does not create custom type

#### Scenario: Name format validation

Given LLM response with `name: "invalid name with spaces"`
When validating
Then system normalizes to kebab-case or rejects
And logs warning about format

