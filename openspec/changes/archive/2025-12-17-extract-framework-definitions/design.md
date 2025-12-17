# Design: Framework Extraction

## Context

Currently, framework-specific knowledge is embedded in three locations:

1. **Language files** (`src/languages/*.rs`):
   - `health_check_patterns()` - Regex patterns for framework-specific health routes
   - `port_patterns()` - Regex patterns for framework-specific port declarations
   - `default_health_endpoints()` - Hardcoded framework defaults

2. **Pipeline phases** (`src/pipeline/phases/`):
   - `06_runtime.rs` - LLM prompt lists 12+ frameworks for detection
   - `12_health.rs` - `try_framework_defaults()` has hardcoded framework→endpoint mapping

3. **Extractors** (`src/extractors/`):
   - `health.rs` - `apply_framework_defaults()` queries language patterns

This scattering makes it impossible to:
- Optimize build templates per framework (Spring Boot fat JAR vs standard JAR)
- Detect frameworks from dependencies alone (must rely on LLM)
- Declare explicit Language-Framework-BuildSystem relationships
- Reuse framework knowledge across different languages (e.g., Ktor for Java+Kotlin)

### Current Architecture (Problem)

```
Language
├── health_check_patterns() -> Vec<(regex, "Spring")>
├── port_patterns() -> Vec<(regex, "Express")>
└── default_health_endpoints() -> Vec<("/actuator/health", "Spring Boot")>

Pipeline Phase 06 (Runtime)
├── LLM prompt: "framework: nextjs | express | spring-boot | ..."
└── RuntimeInfo { framework: Option<String> }  // Just a string!

Pipeline Phase 12 (Health)
├── try_framework_defaults(runtime: &RuntimeInfo)
└── match runtime.framework {
      Some("spring-boot") => "/actuator/health",
      Some("express") => "/health",
      ...
    }
```

**Problems**:
1. Framework is just a string (no type safety, no compile-time validation)
2. Framework defaults duplicated in language files and health phase
3. No way to detect framework from dependencies (Maven sees `spring-boot-starter-web`, but can't infer Spring Boot)
4. No relationship model: Is Spring Boot compatible with Gradle? (Yes, but not declared)

## Goals

1. **Extract frameworks as first-class entities** with traits and registry
2. **Enable deterministic framework detection** from dependencies (no LLM needed for Spring Boot, Next.js, etc.)
3. **Declare Language-Framework-BuildSystem relationships** explicitly
4. **Centralize framework-specific behavior** (health endpoints, ports, build customizations)
5. **Type-safe framework lookup** (no string matching, compile-time validation)
6. **Extensible framework system** (easy to add new frameworks like Ktor, Fastify, etc.)

## Non-Goals

- **NOT changing UniversalBuild schema**: `metadata.framework` remains `Option<String>` (for now)
- **NOT adding framework version detection**: Only detect framework presence (version detection can be added later)
- **NOT framework-specific Dockerfile generation**: Frameworks customize `BuildTemplate`, not generate custom Dockerfiles
- **NOT LLM-based framework detection for unknown frameworks**: Unknown frameworks remain string fallback

## Decisions

### Decision 1: Framework Trait

Define a trait for all frameworks:

```rust
pub trait Framework: Send + Sync {
    fn name(&self) -> &str;
    fn compatible_languages(&self) -> &[&str];
    fn compatible_build_systems(&self) -> &[&str];
    fn dependency_patterns(&self) -> &[DependencyPattern];
    fn default_ports(&self) -> &[u16];
    fn health_endpoints(&self) -> &[&str];
    fn env_var_patterns(&self) -> Vec<(&'static str, &'static str)>;
    fn customize_build_template(&self, template: BuildTemplate) -> BuildTemplate;
}
```

**Rationale**:
- **`compatible_languages()`**: Declare which languages this framework supports (e.g., Spring Boot → Java, Kotlin)
- **`compatible_build_systems()`**: Declare which build systems work with this framework (e.g., Spring Boot → Maven, Gradle)
- **`dependency_patterns()`**: Patterns to detect framework from dependencies (e.g., `spring-boot-starter-web` → Spring Boot)
- **`default_ports()`**: Default ports for this framework (e.g., Spring Boot → [8080], Express → [3000])
- **`health_endpoints()`**: Framework-specific health endpoints (e.g., `/actuator/health` for Spring Boot)
- **`env_var_patterns()`**: Framework-specific environment variable patterns
- **`customize_build_template()`**: Modify build template for framework-specific needs (e.g., Spring Boot fat JAR)

**Alternative considered**: Separate traits for detection (`FrameworkDetector`) and behavior (`FrameworkBehavior`)
- **Rejected**: Over-engineered for current needs, single trait is simpler

### Decision 2: DependencyPattern Struct

Define patterns for framework detection:

```rust
pub struct DependencyPattern {
    pub pattern_type: DependencyPatternType,
    pub pattern: String,
    pub confidence: f32,
}

pub enum DependencyPatternType {
    MavenGroupArtifact,   // e.g., "org.springframework.boot:spring-boot-starter-web"
    NpmPackage,           // e.g., "express"
    PypiPackage,          // e.g., "django"
    Regex,                // e.g., r"spring-boot-starter-.*"
}
```

**Rationale**:
- **Type-specific patterns**: Different ecosystems have different dependency naming conventions
- **Confidence scoring**: Some patterns are stronger indicators than others (e.g., `spring-boot-starter-web` is 0.95, `spring-core` is 0.6)
- **Regex fallback**: For complex patterns (e.g., any Spring Boot starter)

**Example usage**:
```rust
impl Framework for SpringBootFramework {
    fn dependency_patterns(&self) -> &[DependencyPattern] {
        &[
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "org.springframework.boot:spring-boot-starter-web".to_string(),
                confidence: 0.95,
            },
            DependencyPattern {
                pattern_type: DependencyPatternType::Regex,
                pattern: r"spring-boot-starter-.*".to_string(),
                confidence: 0.85,
            },
        ]
    }
}
```

### Decision 3: FrameworkRegistry

Centralized registry for framework lookup:

```rust
pub struct FrameworkRegistry {
    frameworks: Vec<Box<dyn Framework>>,
}

impl FrameworkRegistry {
    pub fn new() -> Self;
    pub fn detect_from_dependencies(&self, deps: &DependencyInfo) -> Option<&dyn Framework>;
    pub fn get_by_name(&self, name: &str) -> Option<&dyn Framework>;
    pub fn all_frameworks(&self) -> &[Box<dyn Framework>];
    pub fn validate_compatibility(&self, language: &str, framework: &str, build_system: &str) -> bool;
}
```

**Rationale**:
- **`detect_from_dependencies()`**: Main detection method, matches dependency patterns
- **`get_by_name()`**: Fallback for LLM-detected frameworks
- **`validate_compatibility()`**: Ensure Language-Framework-BuildSystem combination is valid
- **Registry pattern**: Single source of truth, easy to test, mockable

**Detection algorithm**:
1. Iterate through all frameworks
2. For each framework, check if any dependency pattern matches
3. Return framework with highest confidence score
4. If multiple frameworks match, prefer the one with more specific patterns

### Decision 4: Integration with Existing Systems

#### Phase 6a (Runtime) Changes

**Before** (LLM-based):
```rust
let prompt = format!(
    r#"...
    "framework": "nextjs" | "express" | "spring-boot" | "django" | ... (12+ options)
    ...
    "#
);
let response = llm.chat(prompt).await?;
```

**After** (Deterministic):
```rust
// 1. Parse dependencies (already happens in Phase 4)
let deps = parse_dependencies(&manifest_content)?;

// 2. Detect framework from dependencies
let framework = framework_registry.detect_from_dependencies(&deps);

// 3. Use LLM only if framework not detected
let runtime_info = if let Some(fw) = framework {
    RuntimeInfo {
        framework: Some(fw.name().to_string()),
        confidence: Confidence::High,
        ...
    }
} else {
    // Fallback to LLM detection (existing code)
    llm_detect_runtime(&llm, &service).await?
};
```

**Benefits**:
- Deterministic for major frameworks (Spring Boot, Express, Django, Next.js)
- LLM only needed for unknown/niche frameworks
- Higher confidence scores for common frameworks

#### Phase 6g (Health) Changes

**Before** (Hardcoded):
```rust
fn try_framework_defaults(runtime: &RuntimeInfo) -> Option<HealthInfo> {
    match runtime.framework.as_deref()? {
        "spring-boot" => Some(HealthInfo { endpoint: "/actuator/health", ... }),
        "express" => Some(HealthInfo { endpoint: "/health", ... }),
        ...
    }
}
```

**After** (Framework trait):
```rust
fn try_framework_defaults(
    runtime: &RuntimeInfo,
    framework_registry: &FrameworkRegistry,
) -> Option<HealthInfo> {
    let fw = framework_registry.get_by_name(runtime.framework.as_deref()?)?;
    let endpoints = fw.health_endpoints();

    if !endpoints.is_empty() {
        Some(HealthInfo {
            endpoint: endpoints[0].to_string(),
            source: HealthCheckSource::FrameworkDefault(fw.name().to_string()),
            ...
        })
    } else {
        None
    }
}
```

**Benefits**:
- No string matching (type-safe lookup)
- Framework trait provides canonical health endpoints
- Easy to add new frameworks (no phase modification needed)

### Decision 5: Language-Framework-BuildSystem Relationships

**Many-to-Many Relationships**:

| Language | Frameworks | Build Systems |
|----------|-----------|---------------|
| Java | Spring Boot, Quarkus, Micronaut | Maven, Gradle |
| Kotlin | Spring Boot, Quarkus, Ktor | Maven, Gradle |
| JavaScript/TypeScript | Express, Next.js, Nest.js, Fastify | npm, yarn, pnpm, bun |
| Python | Django, Flask, FastAPI | pip, poetry, pipenv |
| Ruby | Rails, Sinatra | bundler |
| PHP | Laravel, Symfony | composer |
| Go | Gin, Echo | go |
| .NET | ASP.NET Core | dotnet |

**Validation**:
```rust
// Ensure Spring Boot can work with Java + Maven
assert!(framework_registry.validate_compatibility("Java", "Spring Boot", "maven"));

// Ensure Next.js cannot work with Python + pip (should fail)
assert!(!framework_registry.validate_compatibility("Python", "Next.js", "npm"));
```

**Example: Spring Boot**
```rust
pub struct SpringBootFramework;

impl Framework for SpringBootFramework {
    fn name(&self) -> &str { "Spring Boot" }

    fn compatible_languages(&self) -> &[&str] {
        &["Java", "Kotlin"]  // Works with both Java and Kotlin
    }

    fn compatible_build_systems(&self) -> &[&str] {
        &["maven", "gradle"]  // Works with both Maven and Gradle
    }

    fn dependency_patterns(&self) -> &[DependencyPattern] {
        &[
            DependencyPattern {
                pattern_type: DependencyPatternType::MavenGroupArtifact,
                pattern: "org.springframework.boot:spring-boot-starter-web".to_string(),
                confidence: 0.95,
            },
        ]
    }

    fn default_ports(&self) -> &[u16] { &[8080] }

    fn health_endpoints(&self) -> &[&str] { &["/actuator/health"] }

    fn customize_build_template(&self, mut template: BuildTemplate) -> BuildTemplate {
        // Spring Boot creates fat JARs, adjust artifact path
        template.artifacts = vec!["target/*.jar".to_string()];
        template
    }
}
```

### Decision 6: Migration Strategy

**Phase A: Create Framework Module (Parallel Work)**
1. Create `src/frameworks/mod.rs` with trait definition
2. Create `src/frameworks/registry.rs`
3. Implement 5 core frameworks (Spring Boot, Express, Django, Rails, ASP.NET Core)

**Phase B: Implement Remaining Frameworks**
1. Add 10+ additional frameworks
2. Define dependency patterns for each
3. Add tests for framework detection

**Phase C: Integrate with Pipeline**
1. Update Phase 6a to use `FrameworkRegistry.detect_from_dependencies()`
2. Update Phase 6g to query framework for health endpoints
3. Update Phase 10 (Port) to query framework for default ports
4. Pass `FrameworkRegistry` through pipeline context

**Phase D: Remove Framework Logic from Languages**
1. Remove `health_check_patterns()` from Language trait
2. Remove `port_patterns()` from Language trait
3. Remove `default_health_endpoints()` from Language trait
4. Compiler will catch all usages

**Phase E: Update Extractors**
1. Update `health.rs` to query FrameworkRegistry
2. Update `port.rs` to query FrameworkRegistry
3. Remove framework-specific logic

## Risks / Trade-offs

### Risk 1: Dependency Pattern Accuracy
**Concern**: Dependency patterns might have false positives (e.g., `spring-core` doesn't mean Spring Boot)

**Mitigation**:
- Use specific patterns (`spring-boot-starter-web` not just `spring`)
- Confidence scoring allows filtering low-confidence matches
- Framework validation ensures language compatibility
- Comprehensive test fixtures validate pattern accuracy

### Risk 2: Unknown Frameworks
**Concern**: New/niche frameworks won't be detected

**Mitigation**:
- LLM fallback for unknown frameworks (existing behavior)
- Framework registry is extensible (easy to add new frameworks)
- Users can contribute framework definitions via PRs

### Risk 3: Framework Complexity
**Concern**: Some frameworks have complex detection logic (e.g., Next.js vs React)

**Mitigation**:
- Next.js has specific dependency (`next` package)
- React is not a framework (it's a library, no detection needed)
- Complex cases use regex patterns in `dependency_patterns()`

### Trade-off: Line Count Increase vs Maintainability
**Before**: Framework logic scattered across 10 language files (~500-800 lines total)
**After**: Centralized framework module (~1,500-2,000 lines)

**Net**: +700 to +1,200 lines

**Rationale**: Slight line increase acceptable for massive maintainability improvement. Adding new framework now requires 80-120 lines instead of modifying 3-5 files.

## Open Questions

**Q1: Should frameworks provide build commands?**
**A**: No. Build systems provide build commands, frameworks only customize build templates. Separation of concerns: build system = how to build, framework = what to build.

**Q2: Should we detect framework versions?**
**A**: Not in initial implementation. Version detection can be added later via `detect_version()` method. Focus on framework presence first.

**Q3: How to handle framework-specific base images?**
**A**: Frameworks don't change base images. Base images are determined by language runtime + build system. Frameworks only customize build/runtime templates.

## References

- Existing language implementations: `src/languages/*.rs`
- Build system trait: `src/build_systems/mod.rs`
- Pipeline phases: `src/pipeline/phases/06_runtime.rs`, `src/pipeline/phases/12_health.rs`
- Health extractor: `src/extractors/health.rs`
