# Tasks: Unify Registry Chain with Strong Typing

## Phase A: Create Stack Module (Foundation)

### 1. Create Stack Module Structure
- [ ] 1.1 Create `src/stack/` directory
- [ ] 1.2 Create `src/stack/mod.rs` with module declaration
- [ ] 1.3 Add `pub mod stack;` to `src/lib.rs`
- [ ] 1.4 Create `src/stack/registry.rs` (empty stub)
- [ ] 1.5 Create `src/stack/detection.rs` (empty stub)

### 2. Define Typed Identifier Enums
- [ ] 2.1 Define `LanguageId` enum in `src/stack/mod.rs`
  - All 12 languages: Rust, Java, Kotlin, JavaScript, TypeScript, Python, Go, DotNet, Ruby, PHP, Cpp, Elixir
  - Derive: Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize
  - Add serde renames for compatibility (e.g., `csharp`, `c++`)
- [ ] 2.2 Define `BuildSystemId` enum
  - All 16 build systems: Cargo, Maven, Gradle, Npm, Yarn, Pnpm, Bun, Pip, Poetry, Pipenv, GoMod, DotNet, Composer, Bundler, CMake, Mix
- [ ] 2.3 Define `FrameworkId` enum
  - All 20 frameworks: SpringBoot, Quarkus, Micronaut, Ktor, Express, NextJs, NestJs, Fastify, Django, Flask, FastApi, Rails, Sinatra, ActixWeb, Axum, Gin, Echo, AspNetCore, Laravel, Phoenix
- [ ] 2.4 Implement `name()` method for each enum (returns &'static str)
- [ ] 2.5 Add unit tests for enum serialization/deserialization

### 3. Define DetectionStack Structure
- [ ] 3.1 Create `DetectionStack` struct in `src/stack/detection.rs`
  - Fields: build_system, language, framework (Option), confidence, manifest_path
- [ ] 3.2 Implement `validate()` method
- [ ] 3.3 Implement `to_metadata()` for string conversion (UniversalBuild output)
- [ ] 3.4 Add unit tests for DetectionStack
- [ ] 3.5 Remove compatibility with LanguageDetection (breaking change)

## Phase B: Update Traits for Typed IDs

### 4. Update LanguageDefinition Trait
- [ ] 4.1 Add `fn id(&self) -> LanguageId` to trait in `src/languages/mod.rs`
- [ ] 4.2 Change `fn compatible_build_systems(&self) -> &[&str]` to `&[BuildSystemId]`
- [ ] 4.3 Keep `fn name(&self) -> &str` for display purposes
- [ ] 4.4 Verify trait compiles (implementations will fail - expected)

### 5. Implement LanguageId for All Languages
- [ ] 5.1 Implement `id()` for RustLanguage → LanguageId::Rust
- [ ] 5.2 Implement `id()` for JavaLanguage → LanguageId::Java
- [ ] 5.3 Implement `id()` for JavaScriptLanguage → LanguageId::JavaScript
- [ ] 5.4 Implement `id()` for PythonLanguage → LanguageId::Python
- [ ] 5.5 Implement `id()` for GoLanguage → LanguageId::Go
- [ ] 5.6 Implement `id()` for DotNetLanguage → LanguageId::DotNet
- [ ] 5.7 Implement `id()` for RubyLanguage → LanguageId::Ruby
- [ ] 5.8 Implement `id()` for PhpLanguage → LanguageId::PHP
- [ ] 5.9 Implement `id()` for CppLanguage → LanguageId::Cpp
- [ ] 5.10 Implement `id()` for ElixirLanguage → LanguageId::Elixir

### 6. Update BuildSystem Trait
- [ ] 6.1 Add `fn id(&self) -> BuildSystemId` to trait in `src/build_systems/mod.rs`
- [ ] 6.2 Keep `fn name(&self) -> &str` for display

### 7. Implement BuildSystemId for All Build Systems
- [ ] 7.1 Implement `id()` for CargoBuildSystem → BuildSystemId::Cargo
- [ ] 7.2 Implement `id()` for MavenBuildSystem → BuildSystemId::Maven
- [ ] 7.3 Implement `id()` for GradleBuildSystem → BuildSystemId::Gradle
- [ ] 7.4 Implement `id()` for NpmBuildSystem → BuildSystemId::Npm
- [ ] 7.5 Implement `id()` for YarnBuildSystem → BuildSystemId::Yarn
- [ ] 7.6 Implement `id()` for PnpmBuildSystem → BuildSystemId::Pnpm
- [ ] 7.7 Implement `id()` for BunBuildSystem → BuildSystemId::Bun
- [ ] 7.8 Implement `id()` for PipBuildSystem → BuildSystemId::Pip
- [ ] 7.9 Implement `id()` for PoetryBuildSystem → BuildSystemId::Poetry
- [ ] 7.10 Implement `id()` for PipenvBuildSystem → BuildSystemId::Pipenv
- [ ] 7.11 Implement `id()` for GoModBuildSystem → BuildSystemId::GoMod
- [ ] 7.12 Implement `id()` for DotNetBuildSystem → BuildSystemId::DotNet
- [ ] 7.13 Implement `id()` for ComposerBuildSystem → BuildSystemId::Composer
- [ ] 7.14 Implement `id()` for BundlerBuildSystem → BuildSystemId::Bundler
- [ ] 7.15 Implement `id()` for CMakeBuildSystem → BuildSystemId::CMake
- [ ] 7.16 Implement `id()` for MixBuildSystem → BuildSystemId::Mix

### 8. Update Framework Trait
- [ ] 8.1 Add `fn id(&self) -> FrameworkId` to trait in `src/frameworks/mod.rs`
- [ ] 8.2 Change `fn compatible_languages(&self) -> &[&str]` to `&[LanguageId]`
- [ ] 8.3 Change `fn compatible_build_systems(&self) -> &[&str]` to `&[BuildSystemId]`
- [ ] 8.4 Keep `fn name(&self) -> &str` for display

### 9. Implement FrameworkId for All Frameworks
- [ ] 9.1 SpringBootFramework → FrameworkId::SpringBoot, compatible_languages: [Java, Kotlin]
- [ ] 9.2 QuarkusFramework → FrameworkId::Quarkus, compatible_languages: [Java, Kotlin]
- [ ] 9.3 MicronautFramework → FrameworkId::Micronaut, compatible_languages: [Java, Kotlin]
- [ ] 9.4 KtorFramework → FrameworkId::Ktor, compatible_languages: [Kotlin]
- [ ] 9.5 ExpressFramework → FrameworkId::Express, compatible_languages: [JavaScript, TypeScript]
- [ ] 9.6 NextJsFramework → FrameworkId::NextJs, compatible_languages: [JavaScript, TypeScript]
- [ ] 9.7 NestJsFramework → FrameworkId::NestJs, compatible_languages: [TypeScript]
- [ ] 9.8 FastifyFramework → FrameworkId::Fastify, compatible_languages: [JavaScript, TypeScript]
- [ ] 9.9 DjangoFramework → FrameworkId::Django, compatible_languages: [Python]
- [ ] 9.10 FlaskFramework → FrameworkId::Flask, compatible_languages: [Python]
- [ ] 9.11 FastApiFramework → FrameworkId::FastApi, compatible_languages: [Python]
- [ ] 9.12 RailsFramework → FrameworkId::Rails, compatible_languages: [Ruby]
- [ ] 9.13 SinatraFramework → FrameworkId::Sinatra, compatible_languages: [Ruby]
- [ ] 9.14 ActixFramework → FrameworkId::ActixWeb, compatible_languages: [Rust]
- [ ] 9.15 AxumFramework → FrameworkId::Axum, compatible_languages: [Rust]
- [ ] 9.16 GinFramework → FrameworkId::Gin, compatible_languages: [Go]
- [ ] 9.17 EchoFramework → FrameworkId::Echo, compatible_languages: [Go]
- [ ] 9.18 AspNetFramework → FrameworkId::AspNetCore, compatible_languages: [DotNet]
- [ ] 9.19 LaravelFramework → FrameworkId::Laravel, compatible_languages: [PHP]
- [ ] 9.20 PhoenixFramework → FrameworkId::Phoenix, compatible_languages: [Elixir]

## Phase C: Implement StackRegistry

### 10. Create StackRegistry Core
- [ ] 10.1 Define `StackRegistry` struct in `src/stack/registry.rs`
  - Fields: build_systems HashMap, languages HashMap, frameworks HashMap
- [ ] 10.2 Implement `new()` constructor (empty registries)
- [ ] 10.3 Implement `register_build_system(Arc<dyn BuildSystem>)`
- [ ] 10.4 Implement `register_language(Arc<dyn LanguageDefinition>)`
- [ ] 10.5 Implement `register_framework(Box<dyn Framework>)`

### 11. Build Relationship Maps
- [ ] 11.1 Add relationship map fields to StackRegistry
  - build_system_to_languages: HashMap<BuildSystemId, Vec<LanguageId>>
  - language_to_build_systems: HashMap<LanguageId, Vec<BuildSystemId>>
  - language_to_frameworks: HashMap<LanguageId, Vec<FrameworkId>>
  - framework_to_languages: HashMap<FrameworkId, Vec<LanguageId>>
- [ ] 11.2 Implement `build_relationship_maps(&mut self)`
  - Iterate languages, populate build_system_to_languages
  - Iterate frameworks, populate language_to_frameworks
- [ ] 11.3 Call `build_relationship_maps()` in `with_defaults()`

### 12. Implement Detection Methods
- [ ] 12.1 Implement `detect_build_system(&self, manifest_path, content) -> Option<BuildSystemId>`
- [ ] 12.2 Implement `detect_language(&self, manifest, content, build_system_id) -> Option<LanguageId>`
- [ ] 12.3 Implement `detect_stack(&self, manifest_path, content) -> Option<DetectionStack>`
  - Call detect_build_system → detect_language → return DetectionStack
- [ ] 12.4 Implement `detect_framework_from_deps(&self, language, deps) -> Option<FrameworkId>`

### 13. Implement Query Methods
- [ ] 13.1 Implement `get_compatible_languages(&self, build_system) -> &[LanguageId]`
- [ ] 13.2 Implement `get_compatible_frameworks(&self, language) -> &[FrameworkId]`
- [ ] 13.3 Implement `get_build_system(&self, id) -> Option<&dyn BuildSystem>`
- [ ] 13.4 Implement `get_language(&self, id) -> Option<&dyn LanguageDefinition>`
- [ ] 13.5 Implement `get_framework(&self, id) -> Option<&dyn Framework>`
- [ ] 13.6 Implement `detect_primary_language(&self, build_system, file_counts) -> Option<LanguageId>`
  - Count files by language extension
  - Return most-used compatible language
  - Handle ties deterministically (lexicographic order)

### 14. Implement Validation Methods
- [ ] 14.1 Implement `validate_stack(&self, build_system, language, framework) -> bool`
- [ ] 14.2 Implement `validate_all_relationships() -> Vec<String>` (returns errors)
- [ ] 14.3 Add unit tests for validation

### 15. Populate with_defaults()
- [ ] 15.1 Register all 16 build systems
- [ ] 15.2 Register all 12 languages
- [ ] 15.3 Register all 20 frameworks
- [ ] 15.4 Call `build_relationship_maps()`
- [ ] 15.5 Add assertion: `validate_all_relationships().is_empty()`

## Phase D: Update Pipeline

### 16. Update PipelineOrchestrator
- [ ] 16.1 Replace `language_registry: LanguageRegistry` with `stack_registry: StackRegistry`
- [ ] 16.2 Remove `framework_registry: FrameworkRegistry`
- [ ] 16.3 Update `new()` constructor to use StackRegistry::with_defaults()
- [ ] 16.4 Update `with_progress_handler()` constructor
- [ ] 16.5 Update `with_heuristic_logger()` constructor

### 17. Update Phase 1 (Scan)
- [ ] 17.1 Pass `&StackRegistry` to scan::execute()
- [ ] 17.2 Add file counting by language extension in BootstrapScanner
  - Count .rs, .java, .kt, .js, .ts, .py, .go, .cs, .rb, .php, .cpp, .ex files
  - Store in HashMap<LanguageId, usize>
- [ ] 17.3 Update BootstrapScanner to use `stack_registry.detect_stack()`
- [ ] 17.4 Use `detect_primary_language()` for multi-language build systems
- [ ] 17.5 Update BootstrapContext to store Vec<DetectionStack> instead of Vec<LanguageDetection>
- [ ] 17.6 Update tests to use new API

### 18. Update Phase 6a (Runtime)
- [ ] 18.1 Pass `&StackRegistry` to runtime::execute()
- [ ] 18.2 Use `stack_registry.detect_framework_from_deps(language_id, &deps)`
- [ ] 18.3 Convert FrameworkId to String for RuntimeInfo
- [ ] 18.4 Update tests

### 19. Update Phase 6e (Port)
- [ ] 19.1 Update port::execute() to use FrameworkId
- [ ] 19.2 Query framework via `stack_registry.get_framework(framework_id)`
- [ ] 19.3 Update tests

### 20. Update Phase 6g (Health)
- [ ] 20.1 Update health::execute() to use FrameworkId
- [ ] 20.2 Replace string matching with `stack_registry.get_framework(framework_id)`
- [ ] 20.3 Update `try_framework_defaults()` to use typed lookup
- [ ] 20.4 Update tests

### 21. Update Remaining Phases
- [ ] 21.1 Update Phase 4 (Dependencies) to use StackRegistry
- [ ] 21.2 Update extractors to query StackRegistry
- [ ] 21.3 Update Phase 15 (Assemble) to convert IDs to strings for output

## Phase E: Update Tests and Validation

### 22. Remove Old Registries and LanguageDetection
- [ ] 22.1 Delete `src/languages/registry.rs` (LanguageRegistry)
- [ ] 22.2 Delete `src/build_systems/registry.rs` (BuildSystemRegistry)
- [ ] 22.3 Delete `src/frameworks/registry.rs` (FrameworkRegistry)
- [ ] 22.4 Remove `pub struct LanguageDetection` from `src/languages/mod.rs`
- [ ] 22.5 Remove `pub use registry::LanguageRegistry` exports
- [ ] 22.6 Search for remaining LanguageDetection usages: `rg "LanguageDetection" --type rust`
- [ ] 22.7 Update any remaining references to use DetectionStack
- [ ] 22.8 Verify compilation succeeds

### 23. Add Stack Validation Tests
- [ ] 23.1 Test: All Java + Maven combinations are valid
- [ ] 23.2 Test: All JavaScript + npm/yarn/pnpm combinations are valid
- [ ] 23.3 Test: Invalid combinations fail (e.g., Python + Express)
- [ ] 23.4 Test: All frameworks validate with their compatible languages
- [ ] 23.5 Test: Multi-language detection (Kotlin/Java Gradle → Kotlin)
- [ ] 23.6 Test: Multi-language detection (TypeScript/JavaScript npm → TypeScript)
- [ ] 23.7 Test: Tie-breaking is deterministic (50 .kt + 50 .java)
- [ ] 23.8 Property test: Random stack validation

### 24. Update Fixture Tests
- [ ] 24.1 Update rust-cargo fixture test to use StackRegistry
- [ ] 24.2 Update node-npm fixture test
- [ ] 24.3 Update java-maven fixture test
- [ ] 24.4 Update all 14 language fixture tests
- [ ] 24.5 Update monorepo fixture tests

### 25. Update Integration Tests
- [ ] 25.1 Update e2e tests to use new StackRegistry API
- [ ] 25.2 Update CLI integration tests
- [ ] 25.3 Ensure recording system works with typed IDs
- [ ] 25.4 Run full test suite: `cargo test`

## Phase F: Documentation and Cleanup

### 26. Update Documentation
- [ ] 26.1 Update CLAUDE.md to document StackRegistry
- [ ] 26.2 Update src/stack/mod.rs with module documentation
- [ ] 26.3 Add examples of using StackRegistry in rustdoc comments
- [ ] 26.4 Document migration from string-based to typed IDs

### 27. Code Cleanup
- [ ] 27.1 Run `cargo fmt` on all modified files
- [ ] 27.2 Run `cargo clippy` and fix all warnings
- [ ] 27.3 Remove any deprecated code or commented-out sections
- [ ] 27.4 Ensure no string-based lookups remain in public APIs

### 28. Final Validation
- [ ] 28.1 Run full test suite: `cargo test` (all tests pass)
- [ ] 28.2 Run clippy: `cargo clippy` (no warnings)
- [ ] 28.3 Test with fixtures: `cargo run -- detect tests/fixtures/rust-cargo`
- [ ] 28.4 Verify recording replay: `AIPACK_RECORDING_MODE=replay cargo test`
- [ ] 28.5 Run format check: `cargo fmt --check`

## Summary

- **Total tasks**: 28 sections, ~150 individual items
- **Estimated effort**: 16-24 hours
- **Critical path**: Phase A → Phase B → Phase C → Phase D
- **Parallelizable**: Phase B (language/framework implementations) can be done concurrently
- **Risk areas**: Phase D (Pipeline integration) requires careful testing
