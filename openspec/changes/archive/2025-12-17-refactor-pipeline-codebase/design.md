# Design: Build System Extraction

## Context

The current implementation embeds build system logic within language definitions via the `build_template()` method. This creates duplication and maintenance burden:

- **Maven logic duplicated** across Java + Kotlin language files
- **npm/yarn/pnpm/bun logic duplicated** across JavaScript + TypeScript
- **Build systems account for 40-50%** of each language file's code
- **No type-safe build system abstraction** - build systems are just strings

Adding a new JVM language (e.g., Scala, Groovy) requires copying 700+ lines of Maven/Gradle logic. Adding TypeScript requires duplicating npm/yarn/pnpm logic from JavaScript.

### Current Architecture (Problem)

```
Language
├── name(), extensions()
├── build_template(build_system: &str) -> BuildTemplate  ⚠️ 300-400 lines
│   ├── if build_system == "maven" { ... }
│   ├── else if build_system == "gradle" { ... }
│   └── ...
└── parse_dependencies() -> DependencyInfo           ⚠️ 100-200 lines
    ├── parse_maven_pom_xml()
    ├── parse_gradle_build()
    └── ...
```

**Problem**: Maven parsing logic in Java, then copy-pasted to Kotlin. Same for npm across JS/TS.

## Goals

1. **Extract build systems as first-class entities** separate from language definitions
2. **Enable many-to-many relationships**: One build system (Maven) → Multiple languages (Java, Kotlin, Scala)
3. **Reduce duplication**: Define Maven once, reusable by all JVM languages
4. **Type-safe selection**: BuildSystemRegistry instead of string matching
5. **Simplify language additions**: New language = 50-100 lines, not 700+

## Non-Goals

- **NOT changing external API**: DetectionService.detect() remains unchanged
- **NOT changing UniversalBuild schema**: Output format unchanged
- **NOT adding new build systems**: Focus on extracting existing 13 build systems
- **NOT optimizing build system detection speed**: Focus on code organization

## Decisions

### Decision 1: BuildSystem Trait

Define a trait for all build systems with clear responsibilities:

```rust
pub trait BuildSystem: Send + Sync {
    fn name(&self) -> &str;
    fn manifest_patterns(&self) -> &[&str];
    fn detect(&self, manifest_name: &str, content: Option<&str>) -> Option<f64>;
    fn build_template(&self, language_runtime: &str) -> BuildTemplate;
    fn cache_paths(&self) -> &[&str];
    fn parse_dependencies(&self, content: &str) -> Result<DependencyInfo>;
    fn is_workspace_root(&self, content: Option<&str>) -> bool;
}
```

**Rationale**:
- **Manifest patterns**: Each build system knows which files it recognizes (pom.xml, build.gradle, package.json)
- **Detection confidence**: Content-based detection (e.g., check for `<project>` in pom.xml for higher confidence)
- **Template generation**: Build system generates its own Dockerfile-like instructions, given the language runtime
- **Dependency parsing**: Build system knows how to parse its own manifest format
- **Workspace detection**: Build system knows if a manifest is a monorepo root (e.g., `[workspace]` in Cargo.toml)

**Alternative considered**: Keep logic in languages, add helper functions
- **Rejected**: Doesn't solve duplication problem, just moves it around

### Decision 2: BuildTemplate Struct

Standardized output from build systems:

```rust
pub struct BuildTemplate {
    pub build_image: String,           // e.g., "maven:3.9-eclipse-temurin-21"
    pub runtime_image: String,         // e.g., "eclipse-temurin:21-jre"
    pub build_packages: Vec<String>,   // e.g., ["git", "ca-certificates"]
    pub runtime_packages: Vec<String>, // e.g., ["curl"]
    pub build_commands: Vec<String>,   // e.g., ["mvn clean package -DskipTests"]
    pub cache_paths: Vec<String>,      // e.g., ["/root/.m2/repository/"]
    pub artifacts: Vec<String>,        // e.g., ["target/*.jar"]
    pub common_ports: Vec<u16>,        // e.g., [8080]
}
```

**Rationale**:
- **Explicit over implicit**: All fields named, no magic
- **Composable**: Pipeline phases can merge templates from multiple sources
- **Validated**: Type-safe construction

### Decision 3: BuildSystemRegistry

Centralized registry for build system lookup:

```rust
pub struct BuildSystemRegistry {
    build_systems: Vec<Box<dyn BuildSystem>>,
}

impl BuildSystemRegistry {
    pub fn new() -> Self;
    pub fn detect(&self, manifest_name: &str, content: Option<&str>) -> Option<&dyn BuildSystem>;
    pub fn get_by_name(&self, name: &str) -> Option<&dyn BuildSystem>;
}
```

**Rationale**:
- **Replaces string matching**: Type-safe lookup instead of `if name == "maven"`
- **Centralized**: Single place to register all build systems
- **Content-based detection**: Uses build system's own detection logic

**Alternative considered**: Static HashMap<&str, Box<dyn BuildSystem>>
- **Rejected**: Less flexible, harder to test, global mutable state

### Decision 4: Simplified Language Trait

Remove build system responsibilities from languages:

```rust
pub trait Language: Send + Sync {
    fn name(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn runtime_name(&self) -> Option<&'static str>;
    fn compatible_build_systems(&self) -> &[&str];  // ⭐ NEW

    // Code analysis only (not build system related)
    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)>;
    fn port_patterns(&self) -> Vec<(&'static str, &'static str)>;
    fn health_check_patterns(&self) -> Vec<(&'static str, &'static str)>;
    fn is_main_file(&self, fs: &dyn FileSystem, path: &Path) -> bool;

    // ⛔ REMOVED: build_template(), parse_dependencies(), manifest_files()
}
```

**Rationale**:
- **Single responsibility**: Languages describe runtime, build systems describe build process
- **Declaration over implementation**: Languages declare compatibility, don't implement logic
- **Reduced size**: Language files drop from 700+ lines to ~350 lines

### Decision 5: Many-to-Many Relationships

| Build System | Used By Languages |
|--------------|-------------------|
| maven        | Java, Kotlin, Scala, Groovy |
| gradle       | Java, Kotlin, Scala, Groovy |
| npm/yarn/pnpm/bun | JavaScript, TypeScript |
| pip/poetry/pipenv | Python |
| cargo        | Rust |
| go           | Go |
| dotnet       | C# |
| composer     | PHP |
| bundler      | Ruby |
| mix          | Elixir |
| cmake        | C++, C |

**Example: Adding Scala**

**Before** (without build system extraction):
```rust
// src/languages/scala.rs (700+ lines)
pub struct ScalaLanguage;

impl Language for ScalaLanguage {
    fn build_template(&self, build_system: &str) -> BuildTemplate {
        match build_system {
            "maven" => { /* 150 lines copied from java.rs */ }
            "gradle" => { /* 150 lines copied from java.rs */ }
            _ => panic!("unsupported")
        }
    }
}
```

**After** (with build system extraction):
```rust
// src/languages/scala.rs (~50 lines)
pub struct ScalaLanguage;

impl Language for ScalaLanguage {
    fn name(&self) -> &str { "Scala" }
    fn extensions(&self) -> &[&str] { &["scala"] }
    fn runtime_name(&self) -> Option<&'static str> { Some("jre") }
    fn compatible_build_systems(&self) -> &[&str] {
        &["maven", "gradle", "sbt"]  // Build systems defined elsewhere
    }

    // Only Scala-specific code analysis...
}
```

**Impact**: 700 lines → 50 lines (93% reduction), Maven/Gradle logic reused

## Risks / Trade-offs

### Risk 1: Increased Indirection
**Concern**: BuildSystemRegistry adds layer of indirection vs direct language method calls

**Mitigation**:
- Performance impact negligible (single registry lookup per detection)
- Improved maintainability outweighs minor indirection cost
- Type-safe lookup prevents runtime string matching bugs

### Risk 2: Breaking Internal APIs
**Concern**: Large refactoring touches 10 language files + pipeline phases

**Mitigation**:
- Compile-time verification catches all broken references
- Incremental implementation (one build system at a time)
- Comprehensive test coverage validates behavior unchanged
- External API (DetectionService.detect()) remains stable

### Risk 3: Complexity of BuildSystem Trait
**Concern**: 7 methods might be too many responsibilities

**Mitigation**:
- Each method serves clear, distinct purpose
- Trait matches real-world build system responsibilities
- Alternative (multiple traits) would be over-engineered

### Trade-off: Line Count Increase
**Before**: 5,448 lines in language files
**After**: ~3,500 lines in language files + 720 lines in build_systems module
**Net**: +120 to -80 lines

**Rationale**: Slight line increase is acceptable for massive maintainability improvement. Adding new language now requires 50-100 lines instead of 700+.

## Migration Plan

### Phase 1: Create Build Systems Module (parallel work)
1. Create `src/build_systems/mod.rs` with trait definition
2. Create `src/build_systems/registry.rs`
3. Implement 13 build systems (one commit each for easy rollback)

### Phase 2: Update BootstrapScanner
1. Add BuildSystemRegistry alongside LanguageRegistry
2. Use BuildSystemRegistry for manifest detection
3. Keep language-based detection as fallback (gradual migration)

### Phase 3: Update Language Trait
1. Add `compatible_build_systems()` method (additive change)
2. Update all 10 language implementations to declare compatibility
3. Validate: `cargo check && cargo test`

### Phase 4: Update Pipeline Phases
1. Update phases to query BuildSystemRegistry
2. Replace `lang.build_template(build_system)` with `registry.get_by_name(build_system).build_template(runtime)`
3. Validate each phase change with `cargo test`

### Phase 5: Remove Old Methods
1. Remove `build_template()` from Language trait (breaking change)
2. Remove `parse_dependencies()` from language implementations
3. Compiler will catch all usages
4. Fix each error by using BuildSystemRegistry instead

### Rollback Plan
Each phase is independently revertible via git:
- Phase 1: Revert build_systems module creation
- Phase 2: Revert scanner changes
- Phase 3: Revert language trait changes
- Phases 4-5: Revert pipeline changes

## Open Questions

**None** - Architecture is well-defined based on existing code patterns. Implementation is straightforward extraction of existing logic into new module structure.

## References

- Existing language implementations: `src/languages/*.rs`
- Bootstrap scanner: `src/bootstrap/scanner.rs`
- Pipeline phases: `src/pipeline/phases/*.rs`
- tidy-petting-sky.md refactoring plan (detailed analysis)
