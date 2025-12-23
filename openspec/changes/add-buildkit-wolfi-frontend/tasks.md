## Phase 1: Wolfi Package Index Infrastructure

### 1. APKINDEX Fetcher and Parser with Version Discovery

**Critical: Must be completed first - all other phases depend on WolfiPackageIndex**

- [ ] 1.1 Create `src/validation/wolfi_index.rs` module
- [ ] 1.2 Implement `fetch_apkindex()` - Download APKINDEX.tar.gz from `packages.wolfi.dev/os/x86_64/APKINDEX.tar.gz`
- [ ] 1.3 Implement local caching with 24-hour TTL (use `~/.cache/aipack/apkindex/`)
- [ ] 1.4 Implement `parse_apkindex()` - Extract tar.gz → tar → APKINDEX file
- [ ] 1.5 Parse APK index format to extract package names (format: `P:package-name`)
- [ ] 1.6 Implement `WolfiPackageIndex` struct:
  - [ ] 1.6a Add `packages: HashSet<String>` field
  - [ ] 1.6b Implement `get_versions(&self, package_prefix: &str) -> Vec<String>`
    - Parse versioned packages matching prefix (e.g., "nodejs" → ["22", "20", "18"])
    - Extract version numbers from package names (e.g., "nodejs-22" → "22")
    - Sort versions in descending order (highest first)
  - [ ] 1.6c Implement `get_latest_version(&self, package_prefix: &str) -> Option<String>`
    - Return highest available version for prefix
    - Return full package name (e.g., "nodejs-22")
  - [ ] 1.6d Implement `has_package(&self, package_name: &str) -> bool`
    - Check if exact package name exists in index
  - [ ] 1.6e Implement `match_version(&self, package_prefix: &str, requested: &str, available: &[String]) -> Option<String>`
    - Find best match for requested version (e.g., "18" matches "nodejs-18")
    - Support major version matching (e.g., "3.11" matches "python-3.11")
- [ ] 1.7 Add dependency: `tar = "0.4"` to Cargo.toml for APKINDEX extraction
- [ ] 1.8 Add unit tests:
  - [ ] 1.8a Test `get_versions()` with mock APKINDEX (nodejs-22, nodejs-20, nodejs-18)
  - [ ] 1.8b Test `get_latest_version()` returns highest version
  - [ ] 1.8c Test `has_package()` for exact matches
  - [ ] 1.8d Test `match_version()` with various version formats
  - [ ] 1.8e Test version parsing edge cases (e.g., "python-3.12", "dotnet-8-runtime")

## Phase 2: Schema Breaking Changes

### 2. Remove Base Image Fields from UniversalBuild Schema

- [ ] 2.1 Remove `base` field from `BuildStage` struct in `src/output/schema.rs`
- [ ] 2.2 Remove `base` field from `RuntimeStage` struct in `src/output/schema.rs`
- [ ] 2.3 Verify `version` field exists and defaults to `"1.0"` (already implemented)
- [ ] 2.4 Keep schema version at `"1.0"` (removal is simplification, not addition)
- [ ] 2.5 Update all schema tests to remove base image fields
- [ ] 2.6 Update Display implementation to not reference base fields
- [ ] 2.7 Update deserialization tests without base fields

### 3. Remove Base Image Fields from BuildTemplate and Update Trait Signature

**Depends on: Phase 1 (WolfiPackageIndex must exist)**

- [ ] 3.1 Remove `build_image` field from `BuildTemplate` struct in `src/stack/buildsystem/mod.rs`
- [ ] 3.2 Remove `runtime_image` field from `BuildTemplate` struct
- [ ] 3.3 Verify `build_packages` and `runtime_packages` fields already exist
- [ ] 3.4 Update `BuildSystem` trait signature:
  - [ ] 3.4a Change `build_template(&self)` → `build_template(&self, wolfi_index: &WolfiPackageIndex, manifest_content: Option<&str>)`
  - [ ] 3.4b Update all BuildSystem implementations to accept new parameters (temporarily return dummy data)
  - [ ] 3.4c Update documentation to explain WolfiPackageIndex usage for version discovery
- [ ] 3.5 Update BuildTemplate documentation to specify Wolfi packages only

### 4. Update BuildSystem Implementations with Dynamic Version Discovery (16 total)

**Depends on: Phase 1 (WolfiPackageIndex) and Section 3 (trait signature update)**

*Update `build_template()` method to use WolfiPackageIndex for dynamic version discovery instead of hardcoded versions.*

**Node.js-based build systems (query wolfi_index for nodejs-* versions):**
- [ ] 4.1 Update `src/stack/buildsystem/npm.rs`
  - [ ] 4.1a Parse `engines.node` from manifest_content (package.json) if provided
  - [ ] 4.1b Query `wolfi_index.get_versions("nodejs")` to get available versions (e.g., [22, 20, 18])
  - [ ] 4.1c Match requested version to available versions, or use `wolfi_index.get_latest_version("nodejs")`
  - [ ] 4.1d Return BuildTemplate with discovered version (e.g., `nodejs-22`)
- [ ] 4.2 Update `src/stack/buildsystem/yarn.rs` (same as npm)
- [ ] 4.3 Update `src/stack/buildsystem/pnpm.rs` (same as npm)
- [ ] 4.4 Update `src/stack/buildsystem/bun.rs`
  - [ ] 4.4a Query `wolfi_index.has_package("bun")` to check if Bun is available in Wolfi
  - [ ] 4.4b If available, use `bun` package; otherwise fallback to `nodejs-*` with latest version

**Python-based build systems (query wolfi_index for python-* versions):**
- [ ] 4.5 Update `src/stack/buildsystem/pip.rs`
  - [ ] 4.5a Parse version from manifest_content if provided (look for `.python-version`, `runtime.txt`, `pyproject.toml` format)
  - [ ] 4.5b Query `wolfi_index.get_versions("python")` to get available versions (e.g., [3.12, 3.11, 3.10])
  - [ ] 4.5c Match requested version or use `wolfi_index.get_latest_version("python")`
  - [ ] 4.5d Check if `py3-pip` exists in index, construct build_packages and runtime_packages
- [ ] 4.6 Update `src/stack/buildsystem/poetry.rs` (same as pip)
- [ ] 4.7 Update `src/stack/buildsystem/pipenv.rs` (same as pip)

**Java-based build systems (query wolfi_index for openjdk-* versions):**
- [ ] 4.8 Update `src/stack/buildsystem/maven.rs`
  - [ ] 4.8a Parse `maven.compiler.source` from manifest_content (pom.xml) if provided
  - [ ] 4.8b Query `wolfi_index.get_versions("openjdk")` to get available versions (e.g., [21, 17, 11])
  - [ ] 4.8c Match requested version or use `wolfi_index.get_latest_version("openjdk")`
  - [ ] 4.8d Construct runtime package with `-jre` suffix (e.g., `openjdk-21-jre`)
- [ ] 4.9 Update `src/stack/buildsystem/gradle.rs`
  - [ ] 4.9a Parse `sourceCompatibility` from manifest_content (build.gradle/build.gradle.kts) if provided
  - [ ] 4.9b Query `wolfi_index.get_versions("openjdk")` for available versions
  - [ ] 4.9c Match requested version or use latest
  - [ ] 4.9d Construct packages with `-jre` suffix for runtime

**Other build systems:**
- [ ] 4.10 Update `src/stack/buildsystem/cargo.rs`
  - [ ] 4.10a Query `wolfi_index.has_package("rust")` to verify Rust availability
  - [ ] 4.10b Return packages: `["rust", "build-base"]`, runtime: `["glibc", "ca-certificates"]`
- [ ] 4.11 Update `src/stack/buildsystem/go_mod.rs`
  - [ ] 4.11a Query `wolfi_index.has_package("go")` to verify Go availability
  - [ ] 4.11b Return packages: `["go"]`, runtime: `["glibc"]`
- [ ] 4.12 Update `src/stack/buildsystem/dotnet.rs`
  - [ ] 4.12a Parse TargetFramework from manifest_content (.csproj) if provided
  - [ ] 4.12b Query `wolfi_index.get_versions("dotnet")` for available versions
  - [ ] 4.12c Match requested version or use latest
  - [ ] 4.12d Construct runtime package with `-runtime` suffix (e.g., `dotnet-8-runtime`)
- [ ] 4.13 Update `src/stack/buildsystem/bundler.rs`
  - [ ] 4.13a Query `wolfi_index.get_versions("ruby")` for available versions
  - [ ] 4.13b Use latest Ruby version, check if `bundler` package exists
- [ ] 4.14 Update `src/stack/buildsystem/composer.rs`
  - [ ] 4.14a Query `wolfi_index.get_versions("php")` for available versions
  - [ ] 4.14b Use latest PHP version, check if `composer` package exists
- [ ] 4.15 Update `src/stack/buildsystem/cmake.rs`
  - [ ] 4.15a Verify `cmake`, `build-base`, `gcc` exist in wolfi_index
  - [ ] 4.15b Return static packages (no version selection needed)
- [ ] 4.16 Update `src/stack/buildsystem/mix.rs`
  - [ ] 4.16a Query `wolfi_index.get_versions("elixir")` for available versions
  - [ ] 4.16b Use latest Elixir version
- [ ] 4.17 Update `src/stack/buildsystem/llm.rs` (LLM-backed build system)
  - [ ] 4.17a Remove `build_image` and `runtime_image` from `BuildSystemInfo` struct
  - [ ] 4.17b Update LLM prompt to request Wolfi package names instead of base images
  - [ ] 4.17c Update prompt with Wolfi package examples:
    - "For Node.js, use packages like: nodejs-22, nodejs-20, nodejs-18"
    - "For Python, use packages like: python-3.12, python-3.11, python-3.10"
    - "For Java, use packages like: openjdk-21, openjdk-17, openjdk-11"
    - "Always specify version-specific packages (e.g., nodejs-22, not nodejs)"
  - [ ] 4.17d Update `build_template()` signature to accept `wolfi_index` and `manifest_content`
  - [ ] 4.17e Update `build_template()` to validate returned packages against `wolfi_index`
  - [ ] 4.17f Update tests to verify LLM returns valid Wolfi packages
  - [ ] 4.17g If LLM returns invalid package names, fallback to latest versions from wolfi_index

### 5. Update Validation Rules

**Depends on: Phase 1 (WolfiPackageIndex must exist for validation)**

- [ ] 5.1 Remove base image validation from `validate_required_fields()` in `src/validation/rules.rs`
- [ ] 5.2 Remove base image validation from `validate_valid_image_name()` in `src/validation/rules.rs`
- [ ] 5.3 Update validator tests without base images (version stays `1.0`)
- [ ] 5.4 Add test for empty build.packages and runtime.packages (should pass, packages optional)

### 6. Update Test Fixtures (Remove Base Images)

*Remove base fields from all fixture universalbuild.json files, keep version "1.0"*

- [ ] 6.1 Update `tests/fixtures/single-language/rust-cargo/universalbuild.json`
- [ ] 6.2 Update `tests/fixtures/single-language/node-npm/universalbuild.json`
- [ ] 6.3 Update `tests/fixtures/single-language/node-yarn/universalbuild.json`
- [ ] 6.4 Update `tests/fixtures/single-language/node-pnpm/universalbuild.json`
- [ ] 6.5 Update `tests/fixtures/single-language/python-pip/universalbuild.json`
- [ ] 6.6 Update `tests/fixtures/single-language/python-poetry/universalbuild.json`
- [ ] 6.7 Update all monorepo fixtures in `tests/fixtures/monorepo/`
- [ ] 6.8 Ensure all fixtures have `"version": "1.0"` field
- [ ] 6.9 Verify all e2e tests pass with updated fixtures

### 7. Update Assemble Phase

**Depends on: Section 3 (BuildSystem trait signature), Section 4 (BuildSystem implementations)**

- [ ] 7.1 Update `src/pipeline/phases/08_assemble.rs` to not populate `build.base` or `runtime.base`
- [ ] 7.2 Ensure `build.packages` and `runtime.packages` are populated from BuildTemplate
- [ ] 7.3 Pass WolfiPackageIndex to BuildSystem.build_template() calls
- [ ] 7.4 Add unit tests for assemble phase without base images

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
