# Change: Extract Framework Definitions

## Why

Framework-specific logic is currently embedded within language definitions and scattered across pattern methods (`health_check_patterns()`, `port_patterns()`, `default_health_endpoints()`). This creates duplication, maintenance burden, and missed optimization opportunities:

1. **Framework knowledge duplicated** across language files:
   - Spring/Spring Boot patterns in Java
   - Express/Next.js patterns in JavaScript
   - Flask/Django patterns in Python
   - Framework defaults scattered in 3 locations (language methods, health phase, extractors)

2. **No explicit Language-Framework-BuildSystem relationship**:
   - Spring Boot works with Maven + Gradle (but this isn't declared anywhere)
   - Next.js typically uses npm/yarn/pnpm (implicit, not modeled)
   - Framework detection happens via string matching in LLM prompts (`runtime_info.framework`)

3. **Framework-specific behavior is implicit**:
   - Health endpoints: `/actuator/health` for Spring Boot (hardcoded in health phase)
   - Default ports: 8080 for Spring, 3000 for Express (embedded in language patterns)
   - Build customizations: Spring Boot uses `spring-boot-maven-plugin` (not leveraged by build systems)

4. **Missed opportunities**:
   - Can't optimize build templates per framework (e.g., Spring Boot fat JAR vs standard JAR)
   - Can't provide framework-specific best practices (e.g., Next.js build output structure)
   - Can't detect framework from dependencies alone (must rely on LLM)

## What Changes

Extract frameworks as first-class entities with explicit relationships to languages and build systems:

### 1. Create Framework Module (`src/frameworks/`)
- Define `Framework` trait with framework-specific behavior
- Define `FrameworkTemplate` struct (extends `BuildTemplate` with framework knowledge)
- Implement 15+ frameworks:
  - **Java/Kotlin**: Spring Boot, Quarkus, Micronaut, Ktor
  - **JavaScript/TypeScript**: Express, Next.js, Nest.js, Fastify
  - **Python**: Django, Flask, FastAPI
  - **Ruby**: Rails, Sinatra
  - **PHP**: Laravel, Symfony
  - **Go**: Gin, Echo
  - **.NET**: ASP.NET Core

### 2. Establish Language-Framework-BuildSystem Relationship
- Frameworks declare compatible languages (`compatible_languages()`)
- Frameworks declare compatible build systems (`compatible_build_systems()`)
- Frameworks provide dependency signatures for detection (`dependency_patterns()`)
- Create `FrameworkRegistry` for lookup and validation

### 3. Move Framework-Specific Logic Out of Languages
- Remove `health_check_patterns()`, `port_patterns()`, `default_health_endpoints()` from `LanguageDefinition` trait
- Move framework detection from LLM prompts to deterministic dependency analysis
- Framework provides `FrameworkTemplate` that enriches `BuildTemplate`

### 4. Update Pipeline Phases
- Phase 6a (Runtime): Use `FrameworkRegistry` for framework detection from dependencies
- Phase 6b (Build): Merge framework-specific build customizations into build template
- Phase 6e (Port): Query framework for default ports
- Phase 6g (Health): Query framework for health endpoints

### 5. Simplify Extractors
- Remove framework-specific patterns from language files
- Extractors query `FrameworkRegistry` for patterns instead

## Impact

### Affected Specs
- **prompt-pipeline**: Phase 6a (Runtime) changes from LLM-based to deterministic framework detection
  - Framework detection via dependencies (deterministic)
  - LLM only used when dependencies are ambiguous
  - Runtime prompt simplified (no framework enumeration needed)

### Affected Code
- **New module**: `src/frameworks/` (~1,500-2,000 lines)
  - `mod.rs` - Framework trait, FrameworkTemplate
  - `registry.rs` - FrameworkRegistry
  - 15+ framework implementations (~80-120 lines each)

- **Languages module** (simplified):
  - `src/languages/mod.rs` - Remove framework-related trait methods
  - `src/languages/*.rs` - Remove pattern methods (~50-100 lines per file removed)

- **Pipeline phases** (updated):
  - `src/pipeline/phases/06_runtime.rs` - Use FrameworkRegistry
  - `src/pipeline/phases/07_build.rs` - Merge framework templates
  - `src/pipeline/phases/10_port.rs` - Query framework defaults
  - `src/pipeline/phases/12_health.rs` - Query framework defaults

- **Extractors** (simplified):
  - `src/extractors/health.rs` - Query FrameworkRegistry
  - `src/extractors/port.rs` - Query FrameworkRegistry
  - `src/extractors/env_vars.rs` - Query FrameworkRegistry (future)

### Breaking Changes
**None** - All changes are internal refactoring. External API (`DetectionService.detect()`) unchanged. Output schema (`UniversalBuild`) unchanged, though `metadata.framework` may be more accurate.

### Migration Path
**None required** - No external API changes. Internal refactoring only.

### Performance Impact
- **Positive**: Framework detection becomes deterministic (no LLM call)
  - Dependency-based detection is O(n) regex matching vs LLM inference
  - Reduces LLM prompts from 7-9 to 6-8 per detection
  - Estimated 10-15% faster detection for Spring Boot/Next.js projects

- **Positive**: Smaller runtime detection prompts (no framework enumeration)
  - Current: Lists 12+ frameworks in prompt (~150 tokens)
  - New: Frameworks detected before phase 6a (~0 tokens)

- **Neutral**: Framework lookup adds registry query overhead
  - O(n) linear search through 15+ frameworks
  - Negligible compared to file I/O and LLM calls

### Risk Assessment
- **Low risk**: Framework trait extraction (compile-time verified)
- **Medium risk**: Dependency pattern accuracy (requires comprehensive testing)
- **Medium-high risk**: Language-Framework-BuildSystem relationship validation
  - Need to ensure all valid combinations are declared
  - Test fixtures should cover all major frameworks

## Timeline
- **Phase A (Framework Module)**: 6-8 hours
  - Create Framework trait, FrameworkTemplate, FrameworkRegistry
  - Implement 5 core frameworks (Spring Boot, Express, Django, Rails, ASP.NET Core)

- **Phase B (Remaining Frameworks)**: 4-6 hours
  - Implement 10+ additional frameworks
  - Add dependency patterns and detection logic

- **Phase C (Integration)**: 4-6 hours
  - Update pipeline phases to use FrameworkRegistry
  - Remove framework logic from languages
  - Simplify extractors

- **Phase D (Testing & Validation)**: 3-4 hours
  - Add framework detection tests
  - Update existing fixtures with framework metadata
  - Validate all Language-Framework-BuildSystem combinations

- **Total**: 17-24 hours

## Success Criteria
✅ All tests pass (`cargo test`)
✅ No clippy warnings
✅ Framework detection is deterministic for major frameworks
✅ LLM prompts reduced by 10-15% (fewer tokens, fewer calls)
✅ All 14 language fixtures validate with correct framework detection
✅ Framework logic removed from language files (~500-800 lines removed)
✅ Recording system works in replay mode
✅ Language-Framework-BuildSystem relationships are explicit and validated
