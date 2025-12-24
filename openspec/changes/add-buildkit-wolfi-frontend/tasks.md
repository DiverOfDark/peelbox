## Phase 1: Wolfi Package Index Infrastructure

### 1. APKINDEX Fetcher and Parser with Version Discovery

**Critical: Must be completed first - all other phases depend on WolfiPackageIndex**

- [x] 1.1 Create `src/validation/wolfi_index.rs` module
- [x] 1.2 Implement `fetch_apkindex()` - Download APKINDEX.tar.gz from `packages.wolfi.dev/os/x86_64/APKINDEX.tar.gz`
- [x] 1.3 Implement local caching with 24-hour TTL (use `~/.cache/aipack/apkindex/`)
- [x] 1.4 Implement `parse_apkindex()` - Extract tar.gz → tar → APKINDEX file
- [x] 1.5 Parse APK index format to extract package names (format: `P:package-name`)
- [x] 1.6 Implement `WolfiPackageIndex` struct:
  - [x] 1.6a Add `packages: HashSet<String>` field
  - [x] 1.6b Implement `get_versions(&self, package_prefix: &str) -> Vec<String>`
    - Parse versioned packages matching prefix (e.g., "nodejs" → ["22", "20", "18"])
    - Extract version numbers from package names (e.g., "nodejs-22" → "22")
    - Sort versions in descending order (highest first) - **FIXED: Proper semantic version comparison**
  - [x] 1.6c Implement `get_latest_version(&self, package_prefix: &str) -> Option<String>`
    - Return highest available version for prefix
    - Return full package name (e.g., "nodejs-22")
  - [x] 1.6d Implement `has_package(&self, package_name: &str) -> bool`
    - Check if exact package name exists in index
  - [x] 1.6e Implement `match_version(&self, package_prefix: &str, requested: &str, available: &[String]) -> Option<String>`
    - Find best match for requested version (e.g., "18" matches "nodejs-18")
    - Support major version matching (e.g., "3.11" matches "python-3.11")
- [x] 1.7 Add dependency: `tar = "0.4"` to Cargo.toml for APKINDEX extraction
- [x] 1.8 Add unit tests:
  - [x] 1.8a Test `get_versions()` with mock APKINDEX (nodejs-22, nodejs-20, nodejs-18)
  - [x] 1.8b Test `get_latest_version()` returns highest version
  - [x] 1.8c Test `has_package()` for exact matches
  - [x] 1.8d Test `match_version()` with various version formats
  - [x] 1.8e Test version parsing edge cases (e.g., "python-3.12", "dotnet-8-runtime")

**Additional Improvements:**
- [x] Added binary cache with bincode for 30x performance improvement (70ms with warm cache)
- [x] Implemented proper semantic version sorting to handle multi-component versions (1.92 > 1.81 > 1.75)
- [x] Added test APKINDEX cache setup for e2e tests with filetime to prevent cache expiry

## Phase 2: Schema Breaking Changes

### 2. Remove Base Image Fields from UniversalBuild Schema

- [x] 2.1 Remove `base` field from `BuildStage` struct in `src/output/schema.rs` (already removed)
- [x] 2.2 Remove `base` field from `RuntimeStage` struct in `src/output/schema.rs` (already removed)
- [x] 2.3 Verify `version` field exists and defaults to `"1.0"` (already implemented)
- [x] 2.4 Keep schema version at `"1.0"` (removal is simplification, not addition)
- [x] 2.5 Update all schema tests to remove base image fields (already correct)
- [x] 2.6 Update Display implementation to not reference base fields (already correct)
- [x] 2.7 Update deserialization tests without base fields (already correct)

### 3. Remove Base Image Fields from BuildTemplate and Update Trait Signature

**Depends on: Phase 1 (WolfiPackageIndex must exist)**

- [x] 3.1 Remove `build_image` field from `BuildTemplate` struct in `src/stack/buildsystem/mod.rs` (already removed)
- [x] 3.2 Remove `runtime_image` field from `BuildTemplate` struct (already removed)
- [x] 3.3 Verify `build_packages` and `runtime_packages` fields already exist
- [x] 3.4 Update `BuildSystem` trait signature:
  - [x] 3.4a Change `build_template(&self)` → `build_template(&self, wolfi_index: &WolfiPackageIndex, manifest_content: Option<&str>)`
  - [x] 3.4b Update all BuildSystem implementations to accept new parameters
  - [x] 3.4c Update documentation to explain WolfiPackageIndex usage for version discovery
- [x] 3.5 Update BuildTemplate documentation to specify Wolfi packages only

### 4. Update BuildSystem Implementations with Dynamic Version Discovery (16 total)

**Depends on: Phase 1 (WolfiPackageIndex) and Section 3 (trait signature update)**

*Update `build_template()` method to use WolfiPackageIndex for dynamic version discovery instead of hardcoded versions.*

**Node.js-based build systems (query wolfi_index for nodejs-* versions):**
- [x] 4.1 Update `src/stack/buildsystem/npm.rs` (using dynamic version discovery)
  - [x] 4.1a Parse `engines.node` from manifest_content (package.json) if provided
  - [x] 4.1b Query `wolfi_index.get_versions("nodejs")` to get available versions
  - [x] 4.1c Match requested version to available versions, or use latest
  - [x] 4.1d Return BuildTemplate with discovered version
- [x] 4.2 Update `src/stack/buildsystem/yarn.rs` (same as npm)
- [x] 4.3 Update `src/stack/buildsystem/pnpm.rs` (same as npm)
- [x] 4.4 Update `src/stack/buildsystem/bun.rs` (using dynamic version discovery)

**Python-based build systems (query wolfi_index for python-* versions):**
- [x] 4.5 Update `src/stack/buildsystem/pip.rs` (using dynamic version discovery)
- [x] 4.6 Update `src/stack/buildsystem/poetry.rs` (same as pip)
- [x] 4.7 Update `src/stack/buildsystem/pipenv.rs` (same as pip)

**Java-based build systems (query wolfi_index for openjdk-* versions):**
- [x] 4.8 Update `src/stack/buildsystem/maven.rs` (using dynamic version discovery)
  - [x] 4.8a Parse `maven.compiler.source` from manifest_content (pom.xml) if provided
  - [x] 4.8b Query `wolfi_index.get_versions("openjdk")` to get available versions
  - [x] 4.8c Match requested version or use latest
  - [x] 4.8d Construct runtime package with `-jre` suffix
- [x] 4.9 Update `src/stack/buildsystem/gradle.rs` (using dynamic version discovery)

**Other build systems:**
- [x] 4.10 Update `src/stack/buildsystem/cargo.rs` (using dynamic version discovery with get_latest_version)
- [x] 4.11 Update `src/stack/buildsystem/go_mod.rs` (using static packages)
- [x] 4.12 Update `src/stack/buildsystem/dotnet.rs` (using dynamic version discovery)
- [x] 4.13 Update `src/stack/buildsystem/bundler.rs` (using dynamic version discovery)
- [x] 4.14 Update `src/stack/buildsystem/composer.rs` (using dynamic version discovery)
- [x] 4.15 Update `src/stack/buildsystem/cmake.rs` (using static packages)
- [x] 4.16 Update `src/stack/buildsystem/mix.rs` (using dynamic version discovery)
- [x] 4.17 Update `src/stack/buildsystem/llm.rs` (LLM-backed build system)
  - [x] 4.17a Remove `build_image` and `runtime_image` from prompt examples
  - [x] 4.17b Update LLM prompt to request Wolfi package names instead of base images
  - [x] 4.17c Update prompt with Wolfi package examples and guidance
  - [x] 4.17d Update `build_template()` signature to accept `wolfi_index` and `manifest_content`
  - [x] 4.17e Update `build_template()` to validate returned packages against `wolfi_index`

### 5. Update Validation Rules

**Depends on: Phase 1 (WolfiPackageIndex must exist for validation)**

- [x] 5.1 Remove base image validation from `validate_required_fields()` (never existed)
- [x] 5.2 Remove base image validation from `validate_valid_image_name()` (never existed)
- [x] 5.3 Update validator tests without base images (already correct, version stays `1.0`)
- [x] 5.4 Verify empty build.packages and runtime.packages are allowed (already correct)

### 6. Update Test Fixtures (Remove Base Images)

*Remove base fields from all fixture universalbuild.json files, keep version "1.0"*

- [x] 6.1 All test fixtures verified - no base fields present
- [x] 6.2 Verified all fixtures use `"version": "1.0"`
- [x] 6.3 Verified e2e tests pass with fixtures (531/532 unit tests pass, e2e tests pass)

### 7. Update Assemble Phase

**Depends on: Section 3 (BuildSystem trait signature), Section 4 (BuildSystem implementations)**

- [x] 7.1 Verify `src/pipeline/phases/08_assemble.rs` doesn't populate base fields (already correct)
- [x] 7.2 Verify `build.packages` and `runtime.packages` are populated from BuildTemplate (lines 127-130, 159-162)
- [x] 7.3 Verify WolfiPackageIndex is passed to BuildSystem.build_template() (line 68)
- [x] 7.4 Unit tests for assemble phase verified working

## Phase 3: Enhanced Wolfi Package Validation

**Depends on: Phase 1 (WolfiPackageIndex implementation complete)**

### 8. Enhanced Wolfi Package Validation Rule

- [ ] 8.1 Create `validate_wolfi_packages()` function in `src/validation/rules.rs`
- [ ] 8.2 Validate all packages in `build.packages` against APKINDEX package list
- [ ] 8.3 Validate all packages in `runtime.packages` against APKINDEX package list
- [ ] 8.4 Implement fuzzy matching for package name suggestions (Levenshtein distance)
- [ ] 8.5 **Version-aware validation**: If user specifies version-less name (e.g., `nodejs`), suggest versioned alternatives
  - [ ] 8.5a Detect version-less package names (no `-<version>` suffix)
  - [ ] 8.5b Search APKINDEX for versioned variants (e.g., `nodejs-22`, `nodejs-20`, `nodejs-18`)
  - [ ] 8.5c Return error: `Package 'nodejs' not found. Did you mean: nodejs-22, nodejs-20, nodejs-18?`
- [ ] 8.6 Return helpful error messages for typos with format: `Package 'pythonn-3.12' not found. Did you mean: python-3.12?`
- [ ] 8.7 Integrate into `Validator::validate()` method
- [ ] 8.8 Add unit tests for validation scenarios:
  - [ ] 8.8a Valid versioned packages (e.g., `nodejs-22`, `python-3.12`)
  - [ ] 8.8b Invalid version-less packages (e.g., `nodejs`, `python`)
  - [ ] 8.8c Typos with fuzzy matching (e.g., `pythonn-3.12` → `python-3.12`)
  - [ ] 8.8d Completely invalid packages with no suggestions
  - [ ] 8.8e Valid generic packages (e.g., `glibc`, `ca-certificates`, `build-base`)

### 9. LLM-Backed Language Trait Wolfi Guidance

*Guide LLM fallback language implementations to use correct Wolfi package names*

**Note**: BuildSystem LLM guidance is now handled in Section 4.17

- [ ] 9.1 Review `src/stack/language/llm.rs` LLM prompt (if it exists) to include Wolfi package examples
- [ ] 9.2 Add Wolfi package name mapping reference in language detection prompts
- [ ] 9.3 Test LLM-discovered languages return valid Wolfi packages

## Phase 4: BuildKit Client Integration

### 10. BuildKit Client Dependency

- [ ] 10.1 Add `buildkit-client = "0.1"` to Cargo.toml
- [ ] 10.2 Add `buildkit-llb = "0.1"` to Cargo.toml for LLB graph generation
- [ ] 10.3 Create `src/buildkit/mod.rs` module structure

### 11. BuildKit Connection Management

- [ ] 11.1 Create `src/buildkit/client.rs` module
- [ ] 11.2 Implement `BuildKitClient` struct wrapping `buildkit_client::Client`
- [ ] 11.3 Support Unix socket connection (`unix:///run/buildkit/buildkitd.sock`)
- [ ] 11.4 Support TCP connection with optional TLS (`tcp://buildkit:1234`)
- [ ] 11.5 Support Docker container connection (`docker-container://buildkitd`)
- [ ] 11.6 Implement BuildKit version check (require v0.11.0+)
- [ ] 11.7 Add connection error handling with helpful messages
- [ ] 11.8 Support `BUILDKIT_HOST` environment variable
- [ ] 11.9 Add unit tests for connection string parsing

### 12. Two-Stage Distroless LLB Graph Generation (Optimized 2-Layer Final Image)

*Note: Distroless is mandatory for all builds, final image has 2 layers (runtime base + app)*

- [ ] 12.1 Create `src/buildkit/llb.rs` module
- [ ] 12.2 Implement `LLBBuilder` struct that generates 2-stage LLB from UniversalBuild
- [ ] 12.3 Generate source operation for `cgr.dev/chainguard/wolfi-base:latest` (hardcoded)

- [ ] 12.4 **Stage 1 (Build)**: Generate build stage
  - [ ] 12.4a Generate source operation for `wolfi-base`
  - [ ] 12.4b Generate exec operations for `apk add --no-cache <build.packages>`
  - [ ] 12.4c Generate copy operations for build context
  - [ ] 12.4d Generate exec operations for build commands
  - [ ] 12.4e Implement cache mount generation with deterministic IDs
  - [ ] 12.4f Name stage as `build` for artifact copying

- [ ] 12.5 **Stage 2 (Distroless Final)**: Generate distroless final stage with 2 layers
  - [ ] 12.5a **Temp Runtime Prep** (internal stage):
    - Generate source operation for `wolfi-base`
    - Generate exec for `apk add --no-cache <runtime.packages>`
    - Generate exec to copy runtime files to `/runtime-root` (excluding /sbin/apk, /var/lib/apk)
    - Or: Generate exec to remove apk and package metadata after install
    - Name stage as `temp-runtime`
  - [ ] 12.5b **Final Image from Scratch**:
    - Generate `FROM scratch` operation
    - Generate `COPY --from=temp-runtime /runtime-root /` → **Layer 1: Runtime base**
    - Generate `COPY --from=build <artifacts> /app` → **Layer 2: App artifacts**
    - Apply metadata labels (aipack version, language, build system)
    - Set CMD/ENTRYPOINT from UniversalBuild runtime.command

- [ ] 12.6 Verify final image has exactly 2 layers (runtime base + app)
- [ ] 12.7 Add unit tests for 2-stage LLB generation
- [ ] 12.8 Verify LLB graph correctness (stage dependencies, layer structure)
- [ ] 12.9 Add test to verify temp-runtime stage is not in final image manifest

### 13. Build Progress Display

- [ ] 13.1 Create `src/buildkit/progress.rs` module
- [ ] 13.2 Implement progress event handler for BuildKit status stream
- [ ] 13.3 Display layer build status (building, cached, done)
- [ ] 13.4 Display cache hit/miss information
- [ ] 13.5 Display final image digest on completion
- [ ] 13.6 Implement `--quiet` mode (errors only)
- [ ] 13.7 Implement `--verbose` mode (full logs)
- [ ] 13.8 Add progress bar for long-running builds

## Phase 4: SBOM and Build Command

### 14. SBOM Generation

- [ ] 14.1 Research BuildKit SBOM attestation API in `buildkit-client`
- [ ] 14.2 Implement SBOM attestation request in build solve options (always enabled)
- [ ] 14.3 Enable `BUILDKIT_SBOM_SCAN_CONTEXT=true` for full scanning
- [ ] 14.4 Configure Syft scanner options (SPDX format)
- [ ] 14.5 Add tests for SBOM attachment to image manifest

### 15. Provenance Generation

- [ ] 15.1 Implement SLSA provenance attestation request
- [ ] 15.2 Include aipack version in provenance metadata
- [ ] 15.3 Include detected language and build system in provenance
- [ ] 15.4 Include git repository URL if available (detect from .git/)

### 16. Build Command CLI with Multi-App Support

- [ ] 16.1 Add `build` subcommand to CLI in `src/cli/commands.rs`
- [ ] 16.2 Implement `--repo` flag for repository root path (required)
- [ ] 16.3 Implement `--spec` flag for UniversalBuild JSON file (required)
- [ ] 16.4 Implement `--image` flag for image name template (required)
  - [ ] 16.4a Support literal names for single-app builds (e.g., `myapp:latest`)
  - [ ] 16.4b Support `{app}` placeholder for multi-app builds (e.g., `myapp-{app}:latest`)
  - [ ] 16.4c Validate image name format (no spaces, valid Docker image name)
- [ ] 16.5 Implement `--app` flag for building specific app from multi-app spec (optional)
- [ ] 16.6 Load spec file and detect if single object or array of apps
- [ ] 16.7 If spec is array and `--app` not provided, build all apps sequentially
  - [ ] 16.7a Replace `{app}` placeholder with `metadata.project_name` for each app
  - [ ] 16.7b If no `{app}` placeholder, error: "Multi-app build requires {app} placeholder in --image"
- [ ] 16.8 If spec is array and `--app` provided, find app by `metadata.project_name`
  - [ ] 16.8a Replace `{app}` placeholder with the app name
  - [ ] 16.8b If no `{app}` placeholder found, use image name as-is
- [ ] 16.9 If spec is single object (not array), use `--image` value as-is
  - [ ] 16.9a If `{app}` placeholder found, replace with `metadata.project_name`
  - [ ] 16.9b If no placeholder, use literal value
- [ ] 16.10 Implement `--output` flag for export type (docker, oci, tar) - local only, no registry push
- [ ] 16.11 Implement `--buildkit` flag for endpoint configuration
- [ ] 16.12 Support `BUILDKIT_HOST` environment variable (default: `unix:///run/buildkit/buildkitd.sock`)
- [ ] 16.13 Implement `--quiet` and `--verbose` flags
- [ ] 16.14 Add help text and examples for build command (single-app, multi-app)
- [ ] 16.15 Document that all builds are distroless (no flag, mandatory)
- [ ] 16.16 Document that builds are local only (no registry push, use docker push separately)
- [ ] 16.17 Validate each UniversalBuild before starting build
- [ ] 16.18 Display helpful error if BuildKit daemon not available
- [ ] 16.19 Display clear progress for multi-app builds ("Building backend (1/3)...")
- [ ] 16.20 Display "Distroless build" indicator in output (informational only)

### 17. Remove Dockerfile Generation

- [ ] 17.1 Remove `src/output/dockerfile.rs` file entirely
- [ ] 17.2 Remove Dockerfile-related imports and tests
- [ ] 17.3 Update documentation to remove Dockerfile references
- [ ] 17.4 Update README.md examples to use `aipack build` instead of Dockerfile

### 18. Integration Testing

- [ ] 18.1 Create integration test for single-app build workflow
- [ ] 18.2 Create integration test for multi-app build workflow
- [ ] 18.3 Test building all apps from multi-app spec
- [ ] 18.4 Test building specific app from multi-app spec with `--app` flag
- [ ] 18.5 **Verify all builds are distroless with 2-layer structure** (mandatory behavior):
  - [ ] 18.5a Verify final image has exactly 2 layers (runtime base + app)
  - [ ] 18.5b Verify built image has no `/sbin/apk` binary
  - [ ] 18.5c Verify built image has no `/bin/sh` shell
  - [ ] 18.5d Verify built image has no `/var/lib/apk` package database
  - [ ] 18.5e Verify image size is ~10-30MB (vs ~50-100MB for wolfi-base)
- [ ] 18.6 Test that app binary exists and is executable in final image
- [ ] 18.7 Test that runtime dependencies (libs) are present in final image
- [ ] 18.8 Test build with local BuildKit daemon (requires buildkitd running)
- [ ] 18.9 Test SBOM and provenance attestation generation
- [ ] 18.10 Test various output types (docker, oci, tar)
- [ ] 18.11 Test error handling for missing BuildKit daemon
- [ ] 18.12 Test error handling for BuildKit version < 0.11.0
- [ ] 18.13 Test cache mount behavior across builds
- [ ] 18.14 Test Wolfi package validation catches invalid package names
- [ ] 18.15 Test error when `--app` specified but not found in spec

### 19. Documentation

- [ ] 19.1 Update README.md with new `build` command
- [ ] 19.2 Add examples for single-app builds
- [ ] 19.3 Add examples for multi-app builds (build all, build specific app)
- [ ] 19.4 Document that all builds are distroless (mandatory, not optional)
- [ ] 19.5 Document distroless characteristics:
  - [ ] 19.5a No package manager (no apk)
  - [ ] 19.5b No shell (no /bin/sh)
  - [ ] 19.5c Ultra-minimal size (~10-30MB)
  - [ ] 19.5d Production-ready by default
  - [ ] 19.5e Optimized 2-layer structure (runtime base + app)
- [ ] 19.6 Document debugging approaches without shell:
  - [ ] 19.6a Using debug sidecar containers
  - [ ] 19.6b Local development tools (not in prod image)
  - [ ] 19.6c BuildKit debug layer inspection
- [ ] 19.7 Document UniversalBuild spec format (single object vs array)
- [ ] 19.8 Document BuildKit daemon setup requirements (v0.11.0+)
- [ ] 19.9 Document Docker Desktop 4.17+ / Docker Engine 23.0+ requirements
- [ ] 19.10 Add Wolfi package name reference table for common languages
- [ ] 19.11 Add examples for CI/CD integration (monorepo workflow)
- [ ] 19.12 Add size comparison section (distroless vs traditional base images)
- [ ] 19.13 Document layer caching benefits (runtime base cached, only app layer rebuilt on code changes)
- [ ] 19.14 Update CLAUDE.md with Wolfi-first architecture and mandatory distroless 2-layer
- [ ] 19.15 Update CHANGELOG.md with breaking changes (base image removal, Dockerfile removal, distroless mandatory)
- [ ] 19.16 Add migration guide for removing base images from existing specs

## Technical Debt Items

### Version Detection Improvements

- [ ] TD-1 Add .nvmrc/.node-version support for deterministic Node.js version detection
  - Current: Node.js build systems use latest version from APKINDEX
  - Improvement: Parse `.nvmrc` or `.node-version` files to select specific Node version
  - Files to update: `src/stack/buildsystem/npm.rs`, `yarn.rs`, `pnpm.rs`, `bun.rs`
  - Benefit: Developers can pin Node.js version in their repository

- [ ] TD-2 Add runtime version detection for each language from manifest files
  - Current: Runtime versions default to latest from APKINDEX
  - Improvement: Parse version constraints from:
    - PHP: `composer.json` → `require.php` field
    - Python: `pyproject.toml` → `requires-python`, `runtime.txt`
    - Ruby: `.ruby-version`, `Gemfile` → `ruby "x.y.z"`
    - Go: `go.mod` → `go 1.21`
    - Java: Already implemented via `pom.xml`/`build.gradle.kts`
  - Files to update: Language-specific build systems in `src/stack/buildsystem/`
  - Benefit: Reproducible builds with exact runtime versions

### Gradle Manifest Priority Fix

- [ ] TD-3 Fix Gradle detection to prefer build.gradle.kts over settings.gradle.kts for version parsing
  - Current: Detection creates entries for both files, sometimes picks wrong one
  - Issue: `settings.gradle.kts` doesn't contain Java version info, only `build.gradle.kts` does
  - Root cause: Manifest selection logic doesn't prioritize by content relevance
  - Files to update: `src/stack/buildsystem/gradle.rs`, detection/structure phases
  - Benefit: Correct Java version detection for all Gradle projects

### Package Validation

- [ ] TD-4 Verify that packages from RuntimeTrait are actually used in final output
  - Current: RuntimeTrait may suggest packages that aren't included in UniversalBuild
  - Investigation needed: Check if `RuntimeTrait::runtime_packages()` is consulted during assembly
  - Files to check: `src/pipeline/phases/08_assemble.rs`, `src/stack/language/mod.rs`
  - Benefit: Ensure all necessary runtime dependencies are present

### Test Infrastructure

- [ ] TD-5 Add LLM backend support for LLM-only tests (deno-fresh, zig-build)
  - Current: 2/69 e2e tests fail because they require LLM backend
  - Missing: Expected JSON files need to be generated with actual LLM output
  - Files affected: `tests/fixtures/single-language/deno-fresh/`, `zig-build/`
  - Options: Either add expected JSONs from LLM run, or mark as `#[ignore]` without LLM
  - Benefit: 100% test pass rate

### Code Quality and Cleanup

- [ ] TD-6 Review and cleanup wolfi_index.rs implementation
  - Current: Functional implementation completed during Phase 1
  - Areas for improvement:
    - [ ] TD-6a Review error handling patterns (currently uses anyhow, consider custom error types)
    - [ ] TD-6b Review code organization (fetch, parse, cache logic could be separated)
    - [ ] TD-6c Add documentation comments for public API methods
    - [ ] TD-6d Review version sorting edge cases (currently handles semantic versions, test with pre-release/build metadata)
    - [ ] TD-6e Consider adding metrics/logging for cache hit rates
    - [ ] TD-6f Review binary cache format (bincode works but consider versioning for future schema changes)
    - [ ] TD-6g Add integration tests with real APKINDEX download (currently unit tests only)
  - Files: `src/validation/wolfi_index.rs`
  - Benefit: Maintainable, well-documented code with better observability

### Architecture Improvements

- [ ] TD-7 Move runtime_packages from BuildSystemTrait to RuntimeTrait
  - Current: BuildTemplate includes both `build_packages` and `runtime_packages`
  - Issue: Runtime packages are a property of the language/runtime, not the build system
  - Architectural concern: Build systems should only know about build-time dependencies
  - Proposed change:
    - [ ] TD-7a Remove `runtime_packages` field from `BuildTemplate` struct
    - [ ] TD-7b Add `runtime_packages(&self, wolfi_index: &WolfiPackageIndex) -> Vec<String>` to `RuntimeTrait`
    - [ ] TD-7c Update `LanguageTrait` implementations to provide runtime packages
    - [ ] TD-7d Update assemble phase to get runtime packages from language trait, not build system
    - [ ] TD-7e Update all BuildSystem implementations to remove runtime_packages
  - Files affected:
    - `src/stack/buildsystem/mod.rs` (BuildTemplate struct)
    - `src/stack/language/mod.rs` (RuntimeTrait)
    - `src/stack/language/*.rs` (all language implementations)
    - `src/pipeline/phases/08_assemble.rs`
  - Benefit: Cleaner separation of concerns, language-specific runtime dependencies
  - Example: Python runtime needs `python-3.14`, Node.js runtime needs `nodejs-25`, independent of build system (pip/poetry vs npm/yarn)
