# Design: Unified Stack Registry with Strong Typing and Self-Validation

## Context

Currently, aipack has three independent registry systems with fundamental design issues:

### Problem 1: Inverted Dependencies

**Current**: Registry orchestrates detection
```rust
impl LanguageRegistry {
    pub fn detect(&self, manifest: &str, ...) -> Option<LanguageDetection> {
        // Registry decides which language matches
        for language in &self.languages {
            if language.detect(manifest, content).is_some() { ... }
        }
    }
}
```

**Issue**: Registry must know detection logic, creating tight coupling.

### Problem 2: Single Language Assumption

```rust
pub struct LanguageDetection {
    pub language: String,      // ❌ Only ONE
    pub build_system: String,
}
```

**Reality**: Gradle projects often have Kotlin + Java, npm projects have TypeScript + JavaScript.

### Problem 3: String-Based Everything

```rust
fn compatible_languages(&self) -> &[&str] {
    &["Java", "Kotlin"]  // ❌ Typo = runtime bug
}
```

No compile-time validation of relationships.

## Goals

1. **Inversion of control**: Each entity validates **itself**, not validated by registry
2. **Support multiple languages/frameworks**: Reflect real-world projects
3. **Type safety**: Compile-time validation with enums
4. **Simplified registry**: Just storage + iteration helpers, no orchestration logic
5. **Natural chain**: BuildSystem → Language(s) → Framework(s)

## Decisions

### Decision 1: `is_me()` Pattern for Self-Validation

Each entity implements a method that answers: "Am I present in this repository?"

```rust
pub trait Language {
    fn id(&self) -> LanguageId;
    
    // "Am I present in this repository?"
    fn is_me(
        &self,
        build_system: BuildSystemId,
        manifest: &str,
        file_counts: &HashMap<String, usize>,  // extension -> count
    ) -> Option<LanguageUsage>;
    
    fn file_extensions(&self) -> &[&str];
    fn compatible_frameworks(&self) -> &[FrameworkId];
}

pub trait Framework {
    fn id(&self) -> FrameworkId;
    
    // "Am I used in this project?"
    fn is_me(
        &self,
        language: LanguageId,
        deps: &DependencyInfo,
    ) -> Option<FrameworkUsage>;
    
    fn compatible_languages(&self) -> &[LanguageId];
    fn dependency_patterns(&self) -> &[DependencyPattern];
}
```

**Rationale**:
- **Decoupling**: Registry doesn't need to know HOW to detect, only WHO to ask
- **Testability**: Can test `KotlinLanguage.is_me()` in isolation
- **Natural**: Each entity knows itself best
- **No circular dependencies**: Language doesn't need to import Registry

**Example Implementation**:
```rust
pub struct KotlinLanguage;

impl Language for KotlinLanguage {
    fn id(&self) -> LanguageId {
        LanguageId::Kotlin
    }
    
    fn is_me(
        &self,
        build_system: BuildSystemId,
        manifest: &str,
        file_counts: &HashMap<String, usize>,
    ) -> Option<LanguageUsage> {
        // Kotlin-specific detection logic
        let kt_files = file_counts.get(".kt").copied().unwrap_or(0);
        
        if kt_files > 0 {
            Some(LanguageUsage {
                language: LanguageId::Kotlin,
                file_count: kt_files,
                is_primary: false,  // Will be computed later
            })
        } else if build_system == BuildSystemId::Gradle 
            && manifest.contains("kotlin") {
            // Kotlin configured but no files yet
            Some(LanguageUsage {
                language: LanguageId::Kotlin,
                file_count: 0,
                is_primary: false,
            })
        } else {
            None
        }
    }
    
    fn file_extensions(&self) -> &[&str] {
        &[".kt"]
    }
    
    fn compatible_frameworks(&self) -> &[FrameworkId] {
        &[FrameworkId::SpringBoot, FrameworkId::Ktor, FrameworkId::Quarkus]
    }
}
```

### Decision 2: Support Multiple Languages and Frameworks

Real projects use multiple languages and frameworks:

```rust
pub struct DetectionStack {
    pub build_system: BuildSystemId,
    pub languages: Vec<LanguageUsage>,      // Multiple!
    pub frameworks: Vec<FrameworkUsage>,    // Multiple!
    pub manifest_path: PathBuf,
}

pub struct LanguageUsage {
    pub language: LanguageId,
    pub file_count: usize,
    pub is_primary: bool,  // Most-used language
}

pub struct FrameworkUsage {
    pub framework: FrameworkId,
    pub language: LanguageId,  // Which language uses this framework
    pub confidence: f32,
}

impl DetectionStack {
    pub fn primary_language(&self) -> Option<LanguageId> {
        self.languages.iter()
            .find(|l| l.is_primary)
            .map(|l| l.language)
    }
    
    pub fn all_language_ids(&self) -> Vec<LanguageId> {
        self.languages.iter().map(|l| l.language).collect()
    }
}
```

**Rationale**:
- **Realistic**: Gradle projects commonly have 70% Kotlin + 30% Java
- **Framework relationships**: Spring Boot can work with both languages
- **Primary language**: Still maintain concept of "main" language for tooling
- **Explicit associations**: FrameworkUsage knows which language uses it

**Example**:
```
Gradle project:
  Languages: [Kotlin (200 files, primary), Java (50 files)]
  Frameworks: [SpringBoot (Java, 0.95), Ktor (Kotlin, 0.85)]
  
  Interpretation:
  - Mixed Kotlin/Java project
  - Uses Spring Boot (Java-based)
  - Uses Ktor (Kotlin HTTP client library)
```

### Decision 3: Registry as Simple Storage

Registry becomes a passive container with iteration helpers:

```rust
pub struct StackRegistry {
    build_systems: HashMap<BuildSystemId, Arc<dyn BuildSystem>>,
    languages: HashMap<LanguageId, Arc<dyn Language>>,
    frameworks: HashMap<FrameworkId, Box<dyn Framework>>,
}

impl StackRegistry {
    // Simple getters
    pub fn get_build_system(&self, id: BuildSystemId) -> Option<&dyn BuildSystem> {
        self.build_systems.get(&id).map(|bs| bs.as_ref())
    }
    
    pub fn get_language(&self, id: LanguageId) -> Option<&dyn Language> {
        self.languages.get(&id).map(|l| l.as_ref())
    }
    
    pub fn get_framework(&self, id: FrameworkId) -> Option<&dyn Framework> {
        self.frameworks.get(&id).map(|f| f.as_ref())
    }
    
    // Detection helpers (iteration + is_me() calls)
    pub fn detect_languages(
        &self,
        build_system: BuildSystemId,
        manifest: &str,
        file_counts: &HashMap<String, usize>,
    ) -> Vec<LanguageUsage> {
        let build_system_impl = match self.get_build_system(build_system) {
            Some(bs) => bs,
            None => return vec![],
        };
        
        let mut languages: Vec<LanguageUsage> = build_system_impl
            .compatible_languages()
            .iter()
            .filter_map(|lang_id| {
                let lang = self.get_language(*lang_id)?;
                lang.is_me(build_system, manifest, file_counts)
            })
            .collect();
        
        // Mark primary language (most files)
        if let Some(primary_idx) = languages
            .iter()
            .enumerate()
            .max_by_key(|(_, l)| l.file_count)
            .map(|(idx, _)| idx)
        {
            languages[primary_idx].is_primary = true;
        }
        
        languages
    }
    
    pub fn detect_frameworks(
        &self,
        languages: &[LanguageUsage],
        deps: &DependencyInfo,
    ) -> Vec<FrameworkUsage> {
        languages
            .iter()
            .flat_map(|lang_usage| {
                let lang = match self.get_language(lang_usage.language) {
                    Some(l) => l,
                    None => return vec![],
                };
                
                lang.compatible_frameworks()
                    .iter()
                    .filter_map(|fw_id| {
                        let fw = self.get_framework(*fw_id)?;
                        fw.is_me(lang_usage.language, deps)
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}
```

**Rationale**:
- **No orchestration logic**: Just storage and iteration
- **Each call is independent**: Can test `detect_languages()` and `detect_frameworks()` separately
- **Natural filtering**: `filter_map()` with `is_me()` is idiomatic Rust
- **Registry is dumb**: All intelligence is in trait implementations

### Decision 4: Typed Enums for All Identifiers

Replace all `String` identifiers with enums:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageId {
    Rust,
    Java,
    Kotlin,
    JavaScript,
    TypeScript,
    Python,
    Go,
    #[serde(rename = "csharp")]
    DotNet,
    Ruby,
    PHP,
    #[serde(rename = "c++")]
    Cpp,
    Elixir,
}

impl LanguageId {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Java => "Java",
            Self::Kotlin => "Kotlin",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::DotNet => "C#",
            Self::Ruby => "Ruby",
            Self::PHP => "PHP",
            Self::Cpp => "C++",
            Self::Elixir => "Elixir",
        }
    }
}
```

**Rationale**:
- **Compile-time safety**: Typos fail to compile
- **Exhaustive matching**: Compiler forces handling all cases
- **Hash + Eq**: Fast HashMap lookups
- **Copy**: Cheap to pass around
- **Serde**: JSON serialization with custom names

### Decision 5: Natural Detection Flow

Detection becomes simple iteration:

```rust
// 1. Detect build system (single)
let build_system_id = stack_registry.detect_build_system(manifest)?;

// 2. Count files by extension (Phase 1 already does this)
let file_counts: HashMap<String, usize> = walkdir(repo_path)
    .filter_map(|e| {
        let path = e.ok()?.path();
        path.extension()?.to_str().map(|ext| format!(".{}", ext))
    })
    .fold(HashMap::new(), |mut map, ext| {
        *map.entry(ext).or_insert(0) += 1;
        map
    });

// 3. Ask each compatible language: "Are you present?"
let languages = stack_registry.detect_languages(
    build_system_id,
    manifest,
    &file_counts,
);

// 4. Ask each compatible framework: "Are you used?"
let frameworks = stack_registry.detect_frameworks(&languages, deps);

// 5. Build result
let stack = DetectionStack {
    build_system: build_system_id,
    languages,
    frameworks,
    manifest_path: manifest_path.to_path_buf(),
};
```

**Rationale**:
- **Linear flow**: Easy to understand and debug
- **No complex orchestration**: Just iterate and call `is_me()`
- **Parallelizable**: Could make `detect_languages()` parallel with `rayon`
- **Testable**: Each step can be tested independently

### Decision 6: Module Organization

Reorganize codebase to group related modules under `src/stack/`:

**Current Structure** (scattered):
```
src/
├── languages/          # 13 files
├── build_systems/      # 18 files
├── frameworks/         # 22 files
└── ... (other modules)
```

**New Structure** (cohesive):
```
src/stack/
├── mod.rs              # StackRegistry, enums (LanguageId, BuildSystemId, FrameworkId)
├── registry.rs         # StackRegistry implementation
├── detection.rs        # DetectionStack, LanguageUsage, FrameworkUsage
├── languages/          # Language implementations
│   ├── mod.rs          # Language trait
│   ├── rust.rs
│   ├── java.rs
│   ├── kotlin.rs       # ← Split from java.rs
│   ├── javascript.rs
│   ├── typescript.rs   # ← Split from javascript.rs
│   ├── python.rs
│   ├── go.rs
│   ├── csharp.rs       # ← Split from dotnet.rs
│   ├── fsharp.rs       # ← Split from dotnet.rs
│   ├── ruby.rs
│   ├── php.rs
│   ├── cpp.rs
│   ├── elixir.rs
│   └── parsers.rs      # DependencyParser implementations
├── build_systems/      # Build system implementations
│   ├── mod.rs          # BuildSystem trait
│   ├── cargo.rs
│   ├── maven.rs
│   ├── gradle.rs
│   ├── npm.rs
│   ├── yarn.rs
│   ├── pnpm.rs
│   ├── bun.rs
│   ├── pip.rs
│   ├── poetry.rs
│   ├── pipenv.rs
│   ├── go_mod.rs
│   ├── dotnet.rs
│   ├── composer.rs
│   ├── bundler.rs
│   ├── cmake.rs
│   └── mix.rs
└── frameworks/         # Framework implementations
    ├── mod.rs          # Framework trait
    ├── spring_boot.rs
    ├── quarkus.rs
    ├── micronaut.rs
    ├── ktor.rs
    ├── express.rs
    ├── nextjs.rs
    ├── nestjs.rs
    ├── fastify.rs
    ├── django.rs
    ├── flask.rs
    ├── fastapi.rs
    ├── rails.rs
    ├── sinatra.rs
    ├── actix.rs
    ├── axum.rs
    ├── gin.rs
    ├── echo.rs
    ├── aspnet.rs
    ├── laravel.rs
    └── phoenix.rs
```

**Rationale**:
- **Cohesion**: All stack detection logic lives in one place
- **Clear boundaries**: `src/stack/` is self-contained module
- **Easier navigation**: Related code grouped together
- **Import simplification**: `use aipack::stack::{LanguageId, BuildSystemId, FrameworkId}`
- **Logical grouping**: Languages, build systems, and frameworks are all part of the "stack" concept

**Public API** (unchanged):
```rust
// Re-export from src/lib.rs
pub use stack::{
    LanguageId, BuildSystemId, FrameworkId,
    StackRegistry, DetectionStack,
    LanguageUsage, FrameworkUsage,
};
```

### Decision 7: Migration Strategy

**Phase A: Create Stack Module Structure**
1. Create `src/stack/` directory
2. Create `src/stack/mod.rs` with enums and usage structs
3. Create `src/stack/registry.rs` with StackRegistry (storage only)
4. Create `src/stack/detection.rs` with DetectionStack
5. Create `src/stack/languages/`, `src/stack/build_systems/`, `src/stack/frameworks/` subdirectories

**Phase B: Move and Update Traits**
1. Move `src/languages/mod.rs` → `src/stack/languages/mod.rs` and update Language trait
   - Add `id()` method → LanguageId
   - Add `detect()` method with DetectionContext
   - Change `compatible_frameworks()` to return `&[FrameworkId]`
2. Move `src/build_systems/mod.rs` → `src/stack/build_systems/mod.rs` and update BuildSystem trait
   - Add `id()` method → BuildSystemId
   - Add `detect()` method with DetectionContext
   - Change `compatible_languages()` to return `&[LanguageId]`
3. Move `src/frameworks/mod.rs` → `src/stack/frameworks/mod.rs` and update Framework trait
   - Add `id()` method → FrameworkId
   - Add `detect()` method with DetectionContext
   - Change `compatible_languages()` to return `&[LanguageId]`

**Phase C: Move and Split Language Implementations**
1. Move `src/languages/*.rs` → `src/stack/languages/*.rs` (except registry.rs)
2. Split `java.rs` → `java.rs` + `kotlin.rs`
3. Split `javascript.rs` → `javascript.rs` + `typescript.rs`
4. Split `dotnet.rs` → `csharp.rs` + `fsharp.rs` (+ `vbnet.rs` optional)
5. Implement `id()` + `detect()` for all languages

**Phase D: Move Build System and Framework Implementations**
1. Move `src/build_systems/*.rs` → `src/stack/build_systems/*.rs` (except registry.rs)
2. Move `src/frameworks/*.rs` → `src/stack/frameworks/*.rs` (except registry.rs)
3. Implement `id()` + `detect()` for all build systems and frameworks
4. Update framework `compatible_languages()` for split languages

**Phase E: Update Pipeline**
1. Replace three registries with StackRegistry in PipelineOrchestrator
2. Update Phase 1 (Scan) to count files by extension
3. Update Phase 1 to use `detect_languages()` → multiple languages
4. Update Phase 6 (Runtime) to use `detect_frameworks()`
5. Update all phases to work with `Vec<LanguageUsage>` instead of single language
6. Update all imports: `use aipack::stack::{...}`

**Phase F: Cleanup**
1. Delete old directories: `src/languages/`, `src/build_systems/`, `src/frameworks/`
2. Delete old registry files (already moved/rewritten)
3. Delete `LanguageDetection` struct
4. Update `src/lib.rs` to re-export from `stack` module
5. Run `cargo test` and fix any remaining compilation errors

## Risks / Trade-offs

### Risk 1: Multiple Languages Complexity

**Concern**: Phases assume single language, now need to handle multiple

**Mitigation**:
- `DetectionStack::primary_language()` provides single-language fallback
- Most phases use primary language only
- Framework detection uses all languages (natural fit)

### Risk 2: `is_me()` Implementation Complexity

**Concern**: Each language/framework must implement detection logic

**Mitigation**:
- Most implementations are simple (10-20 lines)
- Can reuse helper functions (e.g., `contains_dependency()`)
- Better than centralizing all logic in registry

### Trade-off: Flexibility vs Structure

**Before**: Could add language without recompiling (dynamic strings)
**After**: New language requires enum variant (recompile)

**Rationale**: aipack is a CLI tool. All supported languages are known at compile time. Type safety >> flexibility.

## Real-World Multi-Language Scenarios

Sanity check of all language implementations revealed that multiple languages already handle multiple file extensions, validating the need for multi-language support:

### 1. Java + Kotlin (Confirmed Issue)

**Current Implementation**: `src/languages/java.rs`
```rust
impl LanguageDefinition for JavaLanguage {
    fn file_extensions(&self) -> &[&str] {
        &["java", "kt", "kts"]  // ❌ Handles BOTH Java AND Kotlin
    }
}
```

**Problem**: Cannot distinguish Java-only projects from Kotlin-only or mixed projects.

**Real-World Example**:
```
Gradle project with Spring Boot:
  Files: 200 .kt, 50 .java
  Current detection: "Java" (incorrect, should be Kotlin primary + Java secondary)
  Correct detection: [Kotlin (200, primary), Java (50, secondary)]
  Frameworks: SpringBoot can work with both
```

### 2. JavaScript + TypeScript (Confirmed Issue)

**Current Implementation**: `src/languages/javascript.rs`
```rust
impl LanguageDefinition for JavaScriptLanguage {
    fn file_extensions(&self) -> &[&str] {
        &["js", "mjs", "cjs", "jsx", "ts", "tsx", "mts", "cts"]  // ❌ Handles BOTH
    }
}
```

**Problem**: Modern npm projects routinely mix JavaScript and TypeScript.

**Real-World Example**:
```
npm project with Next.js:
  Files: 150 .ts/.tsx, 30 .js/.jsx
  Current detection: "JavaScript" (incorrect, should be TypeScript primary)
  Correct detection: [TypeScript (150, primary), JavaScript (30, secondary)]
  Frameworks: NextJs, Express work with both
```

### 3. C# + F# + VB.NET (Potential Issue)

**Current Implementation**: `src/languages/dotnet.rs`
```rust
impl LanguageDefinition for DotNetLanguage {
    fn file_extensions(&self) -> &[&str] {
        &["cs", "fs", "vb"]  // ❌ Handles THREE languages
    }
}
```

**Problem**: Less common but possible to mix (e.g., F# for algorithms, C# for APIs).

**Real-World Example**:
```
ASP.NET Core project:
  Files: 180 .cs, 20 .fs
  Current detection: "C#" (technically correct but loses F# usage info)
  Correct detection: [CSharp (180, primary), FSharp (20, secondary)]
  Frameworks: AspNetCore (C#, 0.95)
```

### 4. Single-Language Implementations (No Issues)

Verified the following have no multi-language mixing:
- **Python**: `["py", "pyi", "pyw"]` - all Python variants
- **Go**: `["go"]` - single extension
- **Rust**: `["rs"]` - single extension
- **Ruby**: `["rb"]` - single extension
- **PHP**: `["php"]` - single extension
- **C++**: `["cpp", "cc", "cxx"]` - all C++ variants
- **Elixir**: `["ex", "exs"]` - all Elixir variants

### Required Language Splits

To properly support multi-language detection, these implementations must be split:

1. **JavaLanguage** → Split into:
   - `JavaLanguage` (extensions: `[".java"]`)
   - `KotlinLanguage` (extensions: `[".kt", ".kts"]`)

2. **JavaScriptLanguage** → Split into:
   - `JavaScriptLanguage` (extensions: `[".js", ".mjs", ".cjs", ".jsx"]`)
   - `TypeScriptLanguage` (extensions: `[".ts", ".tsx", ".mts", ".cts"]`)

3. **DotNetLanguage** → Split into (optional):
   - `CSharpLanguage` (extensions: `[".cs"]`)
   - `FSharpLanguage` (extensions: `[".fs", ".fsi", ".fsx"]`)
   - `VBNetLanguage` (extensions: `[".vb"]`)

**Migration Impact**: This split is necessary for the new StackRegistry design and must be included in the implementation plan.

## Open Questions

**Q1: How to handle language tie (50 .kt, 50 .java)?**

**A**: Mark first by enum order (deterministic). Alternatively, check manifest for hints (e.g., `apply plugin: 'kotlin'`).

**Q2: Should FrameworkUsage track ALL matching patterns?**
```rust
pub struct FrameworkUsage {
    pub framework: FrameworkId,
    pub language: LanguageId,
    pub confidence: f32,
    pub matched_patterns: Vec<String>,  // For debugging
}
```

**A**: Not initially. Keep it simple. Can add later if needed for diagnostics.

**Q3: Should we split DotNetLanguage into three languages?**

**A**: Yes, for consistency with Java/Kotlin and JS/TS splits. Even though mixing C#/F#/VB is less common, the split enables proper multi-language detection if it does occur.

## References

- Current registries: `src/languages/registry.rs`, `src/build_systems/registry.rs`, `src/frameworks/registry.rs`
- Pipeline orchestrator: `src/pipeline/orchestrator.rs`
- Related change: `extract-framework-definitions` (archived)
