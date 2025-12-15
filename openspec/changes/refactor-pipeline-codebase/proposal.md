# Change: Refactor Pipeline Codebase

## Why

After migrating from tool-based to pipeline-based detection architecture, the codebase contains significant legacy infrastructure and duplicated code. The old analyzer system (~1,400 lines) is completely unused, build system logic is embedded in language definitions causing duplication, and the Confidence enum is duplicated 11 times across pipeline phases. This technical debt creates confusion, increases maintenance burden, and makes the codebase harder to understand and extend.

## What Changes

This comprehensive refactoring removes dead code, consolidates duplicated types, and introduces cleaner abstractions:

1. **Remove legacy analyzer system** (~1,400 lines of dead code)
   - Delete `src/detection/analyzer.rs` (720 lines)
   - Delete `tests/analyzer_integration.rs` (482 lines)
   - Remove RepositoryContext/GitInfo types (~200 lines)

2. **Consolidate Confidence enum** (remove 11× duplication)
   - Create shared `src/pipeline/confidence.rs`
   - Update all 11 pipeline phase files to use shared type

3. **Extract build systems as first-class entities** (architectural improvement)
   - Create `src/build_systems/` module with BuildSystem trait
   - Implement 13 build systems (Maven, Gradle, npm, yarn, pnpm, bun, pip, poetry, cargo, go, dotnet, pipenv, composer)
   - Simplify language files from 700+ lines to ~350 lines each
   - Build systems become reusable across languages (e.g., Maven for Java + Kotlin)

4. **Simplify infrastructure** (~374 lines)
   - Remove PipelineContext/PipelineConfig dependency injection container
   - Remove ExtractorRegistry unnecessary wrapper
   - Flatten validation trait system to direct functions
   - Simplify progress reporting (CLI-only, remove trait)

5. **Consolidate language modules** (~900-1,100 lines)
   - Extract common dependency parsers (TomlDependencyParser, JsonDependencyParser, RegexDependencyParser)
   - Create `impl_language!` macro for boilerplate reduction
   - Consolidate pattern methods (env_var_patterns, port_patterns, health_check_patterns)

6. **Add test fixtures** for untested languages
   - Ruby (Bundler)
   - PHP (Composer)
   - C++ (CMake)
   - Elixir (Mix)

### Net Impact

- **Lines removed**: ~2,780 lines (35% of codebase)
- **Lines added**: ~1,200 lines (build systems module, shared utilities, test fixtures)
- **Net reduction**: ~1,600 lines (20% of codebase)
- **Complexity reduction**: 5 abstraction layers removed, 1 clean abstraction added

## Impact

### Affected Specs
- **prompt-pipeline**: Internal implementation optimizations (external behavior unchanged)
  - Deterministic parsing now uses shared parser modules
  - Build system detection uses BuildSystemRegistry
  - Pipeline phases use shared Confidence enum

### Affected Code
- **Core modules**:
  - `src/detection/` - Remove analyzer.rs, simplify service.rs
  - `src/pipeline/` - Remove context.rs, config.rs, add confidence.rs
  - `src/build_systems/` - New module (720 lines)
  - `src/languages/` - Simplify all 10 language files
  - `src/extractors/` - Remove registry.rs, add common.rs
  - `src/bootstrap/` - Use BuildSystemRegistry
  - `src/validation/` - Flatten trait system
  - `src/progress/` - Remove trait, keep LoggingHandler

- **Tests**:
  - `tests/analyzer_integration.rs` - Delete (tests dead code)
  - `tests/fixtures/` - Add 4 new language fixtures
  - `tests/fixtures/expected/` - Add 4 new expected outputs

### Breaking Changes
**None** - All changes are internal refactoring. External API (DetectionService.detect()) unchanged.

### Migration Path
**None required** - No external API changes. Internal refactoring only.

### Performance Impact
- **Positive**: Registry optimization (O(n²) → O(n) deduplication)
- **Positive**: Scanner optimization (early-exit traversal, proper gitignore)
- **Neutral**: Build system extraction adds indirection but improves maintainability

### Risk Assessment
- **Low risk**: Dead code removal, type consolidation (compile-time verified)
- **Medium risk**: Infrastructure simplification (incremental with tests)
- **Medium-high risk**: Build system extraction, language consolidation (extensive testing required)

## Timeline
- **Phase A (Quick Wins)**: Stages 1-7 = 7.5-11.5 hours
- **Phase B (Architectural)**: Stage 8 = 8-12 hours
- **Phase C (Consolidation)**: Stage 9 = 4-6 hours
- **Total**: 20-30 hours

## Success Criteria
✅ All tests pass (`cargo test`)
✅ All 14 language fixtures validate (including 4 new)
✅ No dead code detected (`cargo clippy`)
✅ ~1,600 lines net reduction
✅ Recording system works in replay mode
✅ Performance improvements measurable in scanner/registry
✅ Build systems are first-class, reusable entities
✅ No Confidence enum duplication
