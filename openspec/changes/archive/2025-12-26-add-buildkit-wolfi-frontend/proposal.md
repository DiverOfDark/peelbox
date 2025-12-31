# Change: Add BuildKit Frontend with Wolfi-First Architecture

## Why

peelbox currently uses traditional base images (debian:bookworm-slim, rust:1.75, node:20) hardcoded in BuildTemplate. This approach has critical limitations:

1. **Security**: Traditional base images contain unnecessary packages, increasing attack surface and CVE exposure
2. **Supply Chain**: No built-in SBOM generation or provenance tracking
3. **Architecture Coupling**: Base images hardcoded in BuildSystem implementations, making Wolfi support a "side feature"
4. **Dockerfile Lock-In**: Dockerfile generation prevents native BuildKit integration and LLB optimization
5. **Image Size**: Debian-based images are significantly larger than necessary

Wolfi/Chainguard images are purpose-built for containers with minimal attack surface, glibc compatibility, and daily security updates. **This change makes Wolfi the first-class citizen, not an optional alternative.**

## What Changes

### 1. **Wolfi-First UniversalBuild Schema** - Remove base image fields entirely
   - **BREAKING**: Remove `build.base` and `runtime.base` from UniversalBuild schema
   - Base image is always `cgr.dev/chainguard/wolfi-base` (hardcoded in BuildKit LLB generation)
   - All dependencies installed via `build.packages` and `runtime.packages` (Wolfi apk packages)
   - **Keep**: Existing `version` field (already exists, defaults to `"1.0"`)
   - **NOTE**: Version field remains `"1.0"` - no schema version bump needed (removal is simplification, not addition)

### 2. **Wolfi-Only BuildTemplate** - Dynamic version discovery from APKINDEX
   - **BREAKING**: Remove `build_image` and `runtime_image` from BuildTemplate struct
   - **BREAKING**: Update `BuildSystem` trait: `build_template(&self, wolfi_index: &WolfiPackageIndex, manifest_content: Option<&str>)`
   - Keep `build_packages` and `runtime_packages` fields (Wolfi package names only, version-specific)
   - BuildSystems query `WolfiPackageIndex` to discover available package versions dynamically
   - Version selection from manifest files where applicable, fallback to latest available in Wolfi
   - **Auto-updates**: When Wolfi adds new runtime versions, peelbox automatically detects them (after 24h cache refresh)
   - Examples:
     - `CargoBuildSystem`: Queries `wolfi_index.has_package("rust")`, returns `["rust", "build-base"]`
     - `NpmBuildSystem`: Queries `wolfi_index.get_versions("nodejs")`, selects from package.json or latest, returns `["nodejs-22"]`
     - `PipBuildSystem`: Queries `wolfi_index.get_versions("python")`, selects from .python-version or latest, returns `["python-3.12", "py3-pip"]`
     - `MavenBuildSystem`: Queries `wolfi_index.get_versions("openjdk")`, selects from pom.xml or latest, returns `["openjdk-21", "maven"]`

### 3. **BuildKit Frontend** - Native BuildKit integration via gRPC using `buildkit-client` crate
   - Direct daemon connection (Unix socket, TCP, Docker container)
   - LLB graph generation internally (always uses wolfi-base)
   - Real-time build progress streaming
   - Local Docker daemon and tarball export support
   - Requires BuildKit v0.11.0+ (Docker Desktop 4.17+ / Docker Engine 23.0+)

### 4. **Enhanced Wolfi Package Validation** - Ensure package names are valid and version-specific
   - Validate `build.packages` and `runtime.packages` against Wolfi APKINDEX
   - Fetches and caches APKINDEX from `packages.wolfi.dev/os/x86_64/APKINDEX.tar.gz`
   - Version-aware validation: Rejects version-less names (e.g., `nodejs`), suggests versioned alternatives (e.g., `nodejs-22`, `nodejs-20`)
   - Fuzzy matching for typos with helpful suggestions
   - Validation integrated into existing `Validator` with new validation rule
   - Examples:
     - ✅ Valid: `nodejs-22`, `python-3.12`, `openjdk-21`
     - ❌ Invalid: `nodejs` → Error: "Did you mean: nodejs-22, nodejs-20, nodejs-18?"
     - ❌ Invalid: `pythonn-3.12` → Error: "Did you mean: python-3.12?"

### 5. **Distroless Final Images** - Optimized 2-layer architecture (always enabled)
   - All builds produce distroless final images (mandatory, not optional)
   - Two-stage build: Build → Distroless final
   - Distroless stage builds custom base layer in single step:
     1. **Layer 1**: Core Wolfi + runtime packages (via apk in temp layer, then copy to scratch)
     2. **Layer 2**: Application artifacts
   - Removes apk package manager and all package metadata from final image
   - Optimized for Docker layer caching (runtime deps + app separate layers)
   - Results in smallest possible image with only app + runtime dependencies

### 6. **SBOM and Provenance** - Supply chain security attestations
   - SPDX format SBOM via BuildKit's Syft scanner
   - SLSA provenance with build metadata
   - Attached to image manifest

### 7. **New `build` Command** - Direct image building with multi-app support
   - `peelbox build --repo /path/to/repo --spec universalbuild.json --image myapp:latest`
   - Multi-app support: Use `{app}` placeholder in image name template (e.g., `--image myapp-{app}:latest`)
   - `--app <name>` flag to build specific app from multi-app spec
   - `--output type=docker|oci|tar` for export options (local only, no registry push)
   - `--buildkit <endpoint>` for daemon configuration
   - **All builds produce distroless images** (no flag needed, always enabled)

   **Multi-app workflow:**
   ```bash
   # Build all apps in spec using template (distroless by default)
   peelbox build --repo . --spec universalbuild.json --image myapp-{app}:latest
   # Produces: myapp-backend:latest, myapp-frontend:latest

   # Build specific app (when multiple apps defined)
   peelbox build --repo . --spec universalbuild.json --app backend --image backend:latest

   # Single-app build
   peelbox build --repo . --spec universalbuild.json --image myapp:latest

   # All images are distroless - smallest possible size with maximum security
   ```

### 8. **Breaking Changes**
   - **REMOVE**: Dockerfile generation (`src/output/dockerfile.rs`)
   - **REMOVE**: Base image fields from UniversalBuild schema (`build.base`, `runtime.base`)
   - **REMOVE**: Base image fields from BuildTemplate struct (`build_image`, `runtime_image`)
   - **UPDATE**: All BuildSystem implementations to return Wolfi packages instead of base images
   - **UPDATE**: All test fixtures to remove base images (schema version remains `1.0`)
   - **MANDATORY**: All builds produce distroless final images (no opt-out)
   - **NOTE**: Schema version stays `1.0` (removal simplifies schema, doesn't add complexity)

## Impact

- **Affected specs:** buildkit-frontend (new), wolfi-first-schema (new), wolfi-package-validation (new), distroless-2layer (new)
- **Affected code:**
  - `src/buildkit/` - **NEW** module for BuildKit client, LLB generation, progress display
  - `src/output/schema.rs` - **BREAKING** Remove `build.base` and `runtime.base` fields (version stays `1.0`)
  - `src/stack/buildsystem/mod.rs` - **BREAKING** Remove `build_image`/`runtime_image` from BuildTemplate, update `build_template()` signature to accept `WolfiPackageIndex`
  - `src/stack/buildsystem/*.rs` - **UPDATE** All 16 build systems to query `WolfiPackageIndex` for dynamic version discovery
  - `src/validation/rules.rs` - **NEW** Enhanced Wolfi package validation with version-awareness and fuzzy matching
  - `src/validation/wolfi_index.rs` - **NEW** APKINDEX fetcher, parser, and version discovery API (`get_versions()`, `get_latest_version()`, `has_package()`)
  - `src/cli/commands.rs` - **NEW** `build` command
  - `src/output/dockerfile.rs` - **REMOVED**
  - `tests/fixtures/*/universalbuild.json` - **UPDATE** Remove base fields, use version-specific packages (keep version `1.0`)
  - `Cargo.toml` - Add `buildkit-client`, `tar` (APKINDEX parsing) dependencies
- **External API:**
  - **NEW**: `peelbox build` command for direct image building with multi-app support
    - `--repo <path>` - Repository root directory
    - `--spec <file>` - UniversalBuild JSON file (can contain single app or array of apps)
    - `--app <name>` - Build specific app from multi-app spec (optional, builds all if omitted)
    - `--image <name>` - Image name template (required)
      - Single-app: `myapp:latest`
      - Multi-app: `myapp-{app}:latest` (use `{app}` placeholder)
    - **All builds produce distroless images** (mandatory, no flag)
  - **REMOVED**: Dockerfile generation (`peelbox detect` no longer outputs Dockerfiles)
  - **BREAKING**: UniversalBuild schema removes base image fields (version stays `1.0`)
- **Breaking changes:**
  - Dockerfile generation removed entirely
  - UniversalBuild schema removes `build.base` and `runtime.base` (version stays `1.0`)
  - Base images hardcoded to Wolfi (no configuration option)
  - All images are distroless (no opt-out, mandatory for security)
  - BuildKit v0.11.0+ required for `build` command
