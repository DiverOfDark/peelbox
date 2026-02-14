# peelbox-wolfi

Lightweight library for dynamic Wolfi package version discovery. Resolves generic package names (e.g., `nodejs`) to versioned Wolfi packages (e.g., `nodejs-22`) using the Wolfi APKINDEX.

## Architecture

Single-file crate (`src/lib.rs`, ~400 lines). One public type: `WolfiPackageIndex`.

## Public API

```rust
// Initialization
WolfiPackageIndex::fetch() -> Result<Self>           // Download + parse with two-tier caching
WolfiPackageIndex::from_file(path: &Path) -> Result<Self>  // Load from file (testing)
WolfiPackageIndex::for_tests() -> Self               // Load test fixture (OnceLock memoized)

// Queries
has_package(&self, name: &str) -> bool               // Exact match (O(1) HashSet lookup)
get_versions(&self, prefix: &str) -> Vec<String>     // All versions, sorted descending by semver
get_latest_version(&self, prefix: &str) -> Option<String>  // Latest versioned package name
match_version(&self, prefix: &str, requested: &str, available: &[String]) -> Option<String>
all_packages(&self) -> Vec<String>                   // Sorted list of all packages (debugging)
```

## Two-Tier Caching

| Tier | File | Speed | TTL |
|------|------|-------|-----|
| Parsed binary | `$CACHE/peelbox/apkindex/packages.bin` (bincode) | ~5ms | Invalidated if tar.gz newer |
| Raw tar.gz | `$CACHE/peelbox/apkindex/APKINDEX.tar.gz` | ~500ms | 24 hours |
| Network download | https://packages.wolfi.dev/os/x86_64/APKINDEX.tar.gz | ~2-5s | On cache miss |

Cache directory: `PEELBOX_CACHE_DIR` env var, or platform default (`dirs::cache_dir()`).

## Important Notes

- **Wolfi packages are versioned in the index**: `nodejs-22`, `python-3.12`, not `nodejs`, `python`
- **`get_versions()` filters out** versions with `-` (package variants) and non-numeric starts
- **Returns descending order** (newest first), not ascending
- **Test fixture required**: Tests panic if `tests/data/APKINDEX.tar.gz` is missing
- **`PEELBOX_CACHE_DIR` overrides completely** -- no fallback to platform dirs if set but invalid

## Used By

- **peelbox-detect** (`pipeline.rs`): `resolve_wolfi_packages()` converts generic -> versioned names
- **peelbox-pipeline** (`validation/rules.rs`): validates all declared packages exist in Wolfi index

## Tests

6 unit tests using `WolfiPackageIndex::for_tests()`. Run with: `cargo test -p peelbox-wolfi`
