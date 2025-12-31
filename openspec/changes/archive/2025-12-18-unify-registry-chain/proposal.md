# Change: Unify Registry Chain with Strong Typing

## Why

The current architecture has three independent registries (LanguageRegistry, BuildSystemRegistry, FrameworkRegistry) with string-based relationships that create several problems:

1. **No type safety**: All relationships use `&[&str]`, allowing invalid combinations to compile:
   ```rust
   // This compiles but makes no sense:
   framework.compatible_languages().contains(&"InvalidLanguage")
   ```

2. **Inverted dependencies**: Registry orchestrates detection instead of entities validating themselves:
   ```rust
   // Current: Registry decides if language matches
   let detection = language_registry.detect(manifest, content, &build_system_registry)?;

   // Better: Language decides if IT matches
   if let Some(usage) = language.is_me(context) { ... }
   ```

3. **Single language assumption**: Current design assumes one language per build system:
   ```rust
   pub struct LanguageDetection {
       pub language: String,  // ❌ Only one
       pub build_system: String,
   }
   // But Gradle projects often have both Kotlin AND Java!
   ```

4. **Manual coordination required**: Detection requires passing all three registries around:
   ```rust
   let build_system = build_system_registry.detect(manifest, content)?;
   let language_detection = language_registry.detect(manifest, content, &build_system_registry)?;
   let framework = framework_registry.detect_from_dependencies(&deps)?;
   ```

5. **No natural chain**: The flow is Manifest → BuildSystem → Language(s) → Framework(s), but this isn't reflected in code.

## What Changes

### 1. Introduce Strong Types for Identifiers

Replace string-based identifiers with typed enums:

```rust
// Before: Strings everywhere
fn compatible_build_systems(&self) -> &[&str];
fn compatible_languages(&self) -> &[&str];

// After: Strong types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust, Java, Kotlin, JavaScript, TypeScript, Python, Go,
    DotNet, Ruby, PHP, Cpp, Elixir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildSystemId {
    Cargo, Maven, Gradle, Npm, Yarn, Pnpm, Bun,
    Pip, Poetry, Pipenv, GoMod, DotNet, Composer,
    Bundler, CMake, Mix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameworkId {
    SpringBoot, Quarkus, Micronaut, Ktor, Express, NextJs,
    NestJs, Fastify, Django, Flask, FastApi, Rails, Sinatra,
    ActixWeb, Axum, Gin, Echo, AspNetCore, Laravel, Phoenix,
}
```

### 2. Self-Validation with `detect()` Pattern

Each entity validates itself using rich detection context:

```rust
pub struct DetectionContext<'a> {
    pub project_path: &'a Path,
    pub fs: &'a dyn FileSystem,
    pub build_system: Option<BuildSystemId>,  // None during build system detection
    pub manifest_content: &'a str,
    pub file_counts: &'a HashMap<String, usize>,
    pub dependencies: Option<&'a DependencyInfo>,
}

// BuildSystem validates ITSELF
pub trait BuildSystem {
    fn id(&self) -> BuildSystemId;

    // Detect if this build system matches
    fn detect(&self, context: &DetectionContext) -> Option<BuildSystemId>;

    fn compatible_languages(&self) -> &[LanguageId];
}

// Language validates ITSELF
pub trait Language {
    fn id(&self) -> LanguageId;

    // Detect if this language is present
    fn detect(&self, context: &DetectionContext) -> Option<LanguageUsage>;

    fn file_extensions(&self) -> &[&str];
    fn compatible_frameworks(&self) -> &[FrameworkId];
}

// Framework validates ITSELF
pub trait Framework {
    fn id(&self) -> FrameworkId;

    // Detect if this framework is used
    fn detect(&self, context: &DetectionContext) -> Option<FrameworkUsage>;

    fn compatible_languages(&self) -> &[LanguageId];
    fn dependency_patterns(&self) -> &[DependencyPattern];
}
```

### 3. Support Multiple Languages and Frameworks

```rust
// Before: Single language per build system
pub struct LanguageDetection {
    pub language: String,
    pub build_system: String,
}

// After: Multiple languages per build system
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
    pub language: LanguageId,  // Which language uses this
    pub confidence: f32,
}
```

### 4. Simplified StackRegistry (Storage Only)

Registry becomes simple storage with iteration helpers:

```rust
pub struct StackRegistry {
    build_systems: HashMap<BuildSystemId, Arc<dyn BuildSystem>>,
    languages: HashMap<LanguageId, Arc<dyn Language>>,
    frameworks: HashMap<FrameworkId, Box<dyn Framework>>,
}

impl StackRegistry {
    // Simple getters
    pub fn get_language(&self, id: LanguageId) -> Option<&dyn Language>;
    pub fn get_framework(&self, id: FrameworkId) -> Option<&dyn Framework>;

    // Detection helpers (just iteration + detect() calls)
    pub fn detect_languages(
        &self,
        context: &DetectionContext,
    ) -> Vec<LanguageUsage> {
        let bs = self.get_build_system(context.build_system)?;

        bs.compatible_languages()
            .iter()
            .filter_map(|lang_id| {
                let lang = self.get_language(*lang_id)?;
                lang.detect(context)
            })
            .collect()
    }
}
```

### 5. Natural Detection Flow

```rust
// 1. Count files by extension
let file_counts = count_files_by_extension(repo_path);

// 2. Create minimal context (build_system = None)
let minimal_context = DetectionContext {
    project_path: repo_path,
    fs: &filesystem,
    build_system: None,  // Don't know yet
    manifest_content,
    file_counts: &file_counts,
    dependencies: None,
};

// 3. Detect build system using same pattern as Language/Framework!
let build_system_id = stack_registry.detect_build_system(&minimal_context)?;

// 4. Parse dependencies (now that we know build system)
let deps = stack_registry.parse_dependencies(build_system_id, manifest_content, repo_path)?;

// 5. Create full context with detected build system
let context = DetectionContext {
    project_path: repo_path,
    fs: &filesystem,
    build_system: Some(build_system_id),
    manifest_content,
    file_counts: &file_counts,
    dependencies: Some(&deps),
};

// 6. Ask each compatible language: "Are you present?"
let languages = stack_registry.detect_languages(&context);
// Returns: [LanguageUsage { Kotlin, 200, primary: true },
//           LanguageUsage { Java, 150, primary: false }]

// 7. Ask each compatible framework: "Are you used?"
let frameworks = stack_registry.detect_frameworks(&context, &languages);
// Returns: [FrameworkUsage { SpringBoot, Java, 0.95 }]

let stack = DetectionStack {
    build_system: build_system_id,
    languages,
    frameworks,
    manifest_path,
};
```

## Impact

### Affected Specs
- **prompt-pipeline**: No changes to behavior, but internal detection logic simplified
  - Phase 1 (Scan) uses StackRegistry and counts files by extension
  - Phase 4 (Dependencies) queries StackRegistry for parsers
  - Phase 6 (Service Analysis) uses `is_me()` pattern for detection

### Affected Code

**New module structure**: `src/stack/` (~600-900 lines + 53 moved files)
```
src/stack/
├── mod.rs              # LanguageId, BuildSystemId, FrameworkId enums, usage structs
├── registry.rs         # StackRegistry implementation (storage + iteration)
├── detection.rs        # DetectionStack struct
├── languages/          # Moved from src/languages/
│   ├── mod.rs          # Language trait (updated)
│   ├── rust.rs         # Moved + updated
│   ├── java.rs         # Split from old java.rs
│   ├── kotlin.rs       # Split from old java.rs
│   ├── javascript.rs   # Split from old javascript.rs
│   ├── typescript.rs   # Split from old javascript.rs
│   ├── csharp.rs       # Split from old dotnet.rs
│   ├── fsharp.rs       # Split from old dotnet.rs
│   └── ... (5 more)
├── build_systems/      # Moved from src/build_systems/
│   ├── mod.rs          # BuildSystem trait (updated)
│   └── ... (16 files moved + updated)
└── frameworks/         # Moved from src/frameworks/
    ├── mod.rs          # Framework trait (updated)
    └── ... (20 files moved + updated)
```

**Module reorganization**:
- Move `src/languages/` → `src/stack/languages/` (13 files → 15 after splits)
- Move `src/build_systems/` → `src/stack/build_systems/` (18 files)
- Move `src/frameworks/` → `src/stack/frameworks/` (22 files)
- Delete old registry files: `languages/registry.rs`, `build_systems/registry.rs`, `frameworks/registry.rs`

**Updated traits** (now in `src/stack/`):
- `languages/mod.rs` - Add `id()` and `detect()` methods, update compatibility to use LanguageId
- `build_systems/mod.rs` - Add `id()` and `detect()` methods, update compatibility to use BuildSystemId
- `frameworks/mod.rs` - Add `id()` and `detect()` methods, update compatibility to use typed IDs
- All include `DetectionContext` parameter with `build_system: Option<BuildSystemId>`

**Language splits** (required for multi-language detection):
- `java.rs` → `java.rs` + `kotlin.rs`
- `javascript.rs` → `javascript.rs` + `typescript.rs`
- `dotnet.rs` → `csharp.rs` + `fsharp.rs` (+ `vbnet.rs` optional)

**All implementations updated** (~53 files total):
- Implement `id()` method returning typed ID
- Implement `detect()` method with DetectionContext
- Replace `&[&str]` with typed enum arrays
- Update framework compatibility for split languages

**Updated pipeline**:
- `src/pipeline/orchestrator.rs` - Replace three registries with StackRegistry
- `src/pipeline/phases/01_scan.rs` - Count files by extension
- `src/pipeline/phases/06_runtime.rs` - Use `detect_frameworks()`
- All phases updated to use DetectionStack with multiple languages

**Tests updated**:
- Registry tests updated to use typed IDs and `is_me()` pattern
- All fixture tests updated for DetectionStack with multiple languages

### Breaking Changes

**LanguageDetection struct removed** - Replaced with `DetectionStack`:

```rust
// REMOVED
pub struct LanguageDetection {
    pub language: String,
    pub build_system: String,
    pub confidence: f64,
    pub manifest_path: String,
}

// REPLACED WITH
pub struct DetectionStack {
    pub build_system: BuildSystemId,
    pub languages: Vec<LanguageUsage>,
    pub frameworks: Vec<FrameworkUsage>,
    pub manifest_path: PathBuf,
}

impl DetectionStack {
    pub fn primary_language(&self) -> Option<LanguageId> {
        self.languages.iter()
            .find(|l| l.is_primary)
            .map(|l| l.language)
    }
}
```

**Files removed/relocated**:
- `src/languages/` directory → moved to `src/stack/languages/`
- `src/build_systems/` directory → moved to `src/stack/build_systems/`
- `src/frameworks/` directory → moved to `src/stack/frameworks/`
- `src/languages/registry.rs` (LanguageRegistry) - deleted
- `src/build_systems/registry.rs` (BuildSystemRegistry) - deleted
- `src/frameworks/registry.rs` (FrameworkRegistry) - deleted

**External API**: `DetectionService.detect()` returns `Vec<UniversalBuild>` unchanged (uses `.name()` methods for string serialization)

**Import changes**:
```rust
// Before
use peelbox::languages::LanguageDefinition;
use peelbox::build_systems::BuildSystem;
use peelbox::frameworks::Framework;

// After
use peelbox::stack::{
    LanguageId, BuildSystemId, FrameworkId,
    StackRegistry, DetectionStack,
};
use peelbox::stack::languages::Language;
use peelbox::stack::build_systems::BuildSystem;
use peelbox::stack::frameworks::Framework;
```

### Migration Path

**Phase A: Create Stack Module Structure**
- Create `src/stack/` directory with subdirectories
- Implement `src/stack/mod.rs` with typed enums (LanguageId, BuildSystemId, FrameworkId)
- Implement `src/stack/registry.rs` with StackRegistry (storage + iteration)
- Implement `src/stack/detection.rs` with DetectionStack and usage structs
- Leave existing directories (`src/languages/`, etc.) intact temporarily

**Phase B: Move and Update Traits**
- Move `src/languages/mod.rs` → `src/stack/languages/mod.rs`
  - Add `id()` method → LanguageId
  - Add `detect()` method with DetectionContext
  - Change `compatible_frameworks()` → `&[FrameworkId]`
- Move `src/build_systems/mod.rs` → `src/stack/build_systems/mod.rs`
  - Add `id()` method → BuildSystemId
  - Add `detect()` method with DetectionContext
  - Change `compatible_languages()` → `&[LanguageId]`
- Move `src/frameworks/mod.rs` → `src/stack/frameworks/mod.rs`
  - Add `id()` method → FrameworkId
  - Add `detect()` method with DetectionContext
  - Change `compatible_languages()` → `&[LanguageId]`

**Phase C: Move and Split Language Implementations**
- Move `src/languages/*.rs` → `src/stack/languages/*.rs` (except registry.rs)
- Split `java.rs` → `java.rs` + `kotlin.rs`
- Split `javascript.rs` → `javascript.rs` + `typescript.rs`
- Split `dotnet.rs` → `csharp.rs` + `fsharp.rs` (+ `vbnet.rs` optional)
- Implement `id()` + `detect()` for all 15 languages

**Phase D: Move Build System and Framework Implementations**
- Move `src/build_systems/*.rs` → `src/stack/build_systems/*.rs` (except registry.rs)
- Move `src/frameworks/*.rs` → `src/stack/frameworks/*.rs` (except registry.rs)
- Implement `id()` + `detect()` for all build systems and frameworks
- Update framework `compatible_languages()` for split languages (Java+Kotlin, JS+TS, C#+F#)

**Phase E: Update Pipeline**
- Replace three registries with StackRegistry in PipelineOrchestrator
- Update Phase 1 (Scan) to count files by extension
- Update all phases to use `DetectionStack` instead of `LanguageDetection`
- Update BootstrapScanner to return `Vec<DetectionStack>` with multiple languages
- Update all imports to use `peelbox::stack::{...}`

**Phase F: Cleanup**
- Delete old directories: `src/languages/`, `src/build_systems/`, `src/frameworks/`
- Delete old registry files (already removed during moves)
- Delete `LanguageDetection` struct
- Update `src/lib.rs` to re-export from `stack` module
- Run `cargo test` and fix any remaining compilation errors

**Phase G: Update Tests**
- Replace all LanguageDetection assertions with DetectionStack
- Add multi-language detection tests (Kotlin+Java, TypeScript+JavaScript, C#+F#)
- Add multi-framework detection tests
- Ensure recording system serializes DetectionStack correctly

**Code search patterns for migration**:
```bash
# Find all LanguageDetection usages
rg "LanguageDetection" --type rust

# Find string-based compatibility checks
rg 'compatible_.*\(\).*contains.*&"' --type rust

# Find registry instantiation
rg "LanguageRegistry|BuildSystemRegistry|FrameworkRegistry" --type rust
```

### Performance Impact

**Positive**:
- Hash lookups with enums are faster than string comparisons
- No redundant compatibility checks (each entity validates itself)
- Natural iteration pattern is cache-friendly

**Neutral**:
- Registry initialization unchanged (just different storage type)
- Memory overhead of typed enums is minimal

### Risk Assessment

**Low risk**:
- Type safety prevents invalid combinations at compile time
- All string-based lookups become enum matches (exhaustive)
- `is_me()` pattern is simple and testable in isolation

**Medium risk**:
- Large refactoring across 40+ files
- Need to ensure all valid combinations are represented in enums
- LanguageDetection removal requires updating all internal callers
- Multiple language support changes assumptions in some phases

## Success Criteria

✅ All tests pass with new typed API
✅ No clippy warnings
✅ Compile-time validation prevents invalid stack combinations
✅ StackRegistry.detect_languages() works for all test fixtures
✅ Multi-language projects correctly detected (Kotlin+Java, TypeScript+JavaScript)
✅ Multi-framework projects correctly detected (SpringBoot+Android)
✅ Pipeline uses single registry instead of three
✅ String-based lookups eliminated from public APIs
✅ All Language-Framework-BuildSystem combinations represented
✅ `detect()` pattern **uniformly** used for self-validation (all three entities)
✅ DetectionContext enables filesystem access for custom env var scanning
✅ No special-casing: BuildSystem uses same `detect()` pattern via `Option<BuildSystemId>`
✅ Performance neutral or improved vs current implementation
