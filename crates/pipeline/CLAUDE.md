# peelbox-pipeline

Thin orchestration crate that wires together detection (peelbox-detect) and
validation. Provides `DetectionService` as the main entry point and `Validator`
for checking `UniversalBuild` specs against Wolfi package availability and
structural rules. Detection is fully deterministic — no LLM, no network beyond
the cached Wolfi APKINDEX.

## Module Structure

```
src/
├── lib.rs                # Re-exports: DetectionService, ServiceError, Validator, WolfiPackageIndex
├── detection/
│   └── service.rs        # DetectionService (orchestration), ServiceError (rich error with help messages)
└── validation/
    ├── validator.rs       # Validator (applies rules in order)
    └── rules.rs           # Validation rules: required fields, non-empty commands, copy specs, Wolfi packages
```

## Key Types

### DetectionService

Main entry point for repository detection. Stateless and deterministic.

```rust
DetectionService::new() -> Self
  .detect(repo_path: PathBuf) -> Result<Vec<UniversalBuild>, ServiceError>
```

Internally:
1. Validates repo path exists and is a directory
2. Fetches `WolfiPackageIndex` (internally cached 24h on disk)
3. Calls `peelbox_detect::detect_with_registry_and_wolfi()`
4. Enforces unique `metadata.project_name` across monorepo results
5. Validates all results with `Validator::with_wolfi_index()`
6. Logs timing via `tracing` (duration_ms, projects_found)

### ServiceError

Rich error type with `help_message()` providing user-friendly guidance:
- `PathNotFound` / `NotADirectory` -- repo path issues
- `ConfigError` -- e.g. duplicate service names across a monorepo
- `DetectionFailed` -- detection, Wolfi fetch, or package validation failures

### Validator

Applies validation rules in order:
1. `validate_required_fields()` -- version, language, build_system must be non-empty
2. `validate_non_empty_commands()` -- build.commands and runtime.command must have entries
3. `validate_valid_copy_specs()` -- all CopySpec.from/to paths must be non-empty
4. `validate_wolfi_packages()` (if WolfiPackageIndex provided) -- checks package existence

Each rule error is prefixed with `[RuleName]` for debugging.

### Wolfi Package Validation

- Collects **all** errors before returning (doesn't short-circuit)
- Fuzzy matching with Levenshtein distance <= 3 for typo suggestions (via `strsim`)
- Detects version-less packages (nodejs, python, openjdk) and suggests versioned alternatives
- First 5 closest matches shown

## Dependencies

Internal: `peelbox-core`, `peelbox-wolfi`, `peelbox-detect`
External: `anyhow`, `thiserror`, `tracing`, `strsim` (fuzzy matching)

## Tests

Run with: `cargo test -p peelbox-pipeline`

- `detection/service.rs`: Path validation tests
- `validation/validator.rs`: Valid/invalid build validation
- `validation/rules.rs`: Individual rule tests including Wolfi fuzzy matching
- Tests use `WolfiPackageIndex::for_tests()` and `tempfile::TempDir`
