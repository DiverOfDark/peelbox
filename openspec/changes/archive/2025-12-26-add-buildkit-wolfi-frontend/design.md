## Context

peelbox generates container build specifications from repository analysis. This design document covers implementing a native BuildKit frontend that connects to the BuildKit daemon, generates secure container images using Wolfi base images, and produces SBOM attestations.

**Stakeholders:**
- Developers using peelbox to containerize applications
- Security teams requiring minimal, auditable container images with SBOM
- Platform teams integrating peelbox into CI/CD pipelines

**Constraints:**
- Wolfi packages may not have 1:1 mapping with traditional packages
- BuildKit daemon must be available for image building
- SBOM scanning adds build time overhead
- Requires BuildKit v0.11.0+ for attestation support

## Goals / Non-Goals

**Goals:**
- Implement native BuildKit frontend with daemon connection via gRPC
- Use `cgr.dev/chainguard/wolfi-base` as the single base image for all builds
- Generate SBOM attestations for supply chain security
- Generate SLSA provenance attestations for build transparency
- Install all runtime dependencies via apk on wolfi-base
- Validate Wolfi package names against the package index
- Provide LLM tools for package discovery and validation

**Non-Goals:**
- Traditional Debian/Ubuntu image support (Wolfi only)
- Custom package building with melange (use existing Wolfi packages)
- Multi-architecture builds in single invocation (BuildKit handles this separately)
- Cryptographic signing of attestations (use Cosign separately if needed)

## Decisions

### Decision 1: Use `buildkit-client` crate for daemon integration

The [`buildkit-client`](https://crates.io/crates/buildkit-client) crate provides a complete gRPC client for BuildKit with session protocol support, progress streaming, and registry authentication.

**Alternatives considered:**
- `buildkit-llb` only (JSON output) - No daemon integration, requires separate tooling
- Raw gRPC with `tonic` - Significant implementation effort
- Shell out to `buildctl` - Loses type safety, harder error handling

**Rationale:** `buildkit-client` provides full BuildKit integration including bidirectional streaming, file sync, and registry push - everything needed for a complete frontend.

### Decision 2: Wolfi-first schema - Remove base image fields entirely

**BREAKING CHANGE**: Remove `build.base` and `runtime.base` fields from UniversalBuild schema. Base image is always `cgr.dev/chainguard/wolfi-base`, hardcoded in BuildKit LLB generation.

**Schema changes:**
- **Remove**: `BuildStage.base` field
- **Remove**: `RuntimeStage.base` field
- **Remove**: `BuildTemplate.build_image` field
- **Remove**: `BuildTemplate.runtime_image` field
- **Keep**: `BuildStage.packages` and `RuntimeStage.packages` (now always Wolfi apk packages)
- **Keep**: `version` field (already exists, defaults to `"1.0"`)
- **No version bump**: Schema version stays `1.0` (removal is simplification, not breaking addition)

**Build/runtime strategy:**
- Build stage: `wolfi-base` + language toolchain packages (e.g., `rust`, `nodejs-22`, `python-3.12`)
- Runtime stage: `wolfi-base` + minimal runtime packages (e.g., `glibc`, `ca-certificates`)

**Alternatives considered:**
- Keep base image fields, make Wolfi default - Adds complexity, allows users to shoot themselves in the foot
- Language-specific Chainguard images (`cgr.dev/chainguard/rust`) - Less flexible, harder to customize
- Distroless runtime images - Cannot install runtime dependencies if needed
- Make base image configurable - Adds configuration complexity, defeats security purpose

**Rationale:**
- **Simplicity**: No configuration needed, one base image for everything
- **Security**: Users cannot accidentally use vulnerable base images
- **Consistency**: All builds use the same tested, secure foundation
- **Breaking change justified**: peelbox is pre-1.0, Wolfi is the future, no need for backwards compatibility

### Decision 3: SBOM and provenance attestations

Generate [SBOM attestations](https://docs.docker.com/build/metadata/attestations/sbom/) in SPDX format and [SLSA provenance](https://docs.docker.com/build/attestations/) for all builds.

**Implementation:**
- Use BuildKit's built-in Syft scanner for SBOM generation
- Enable `BUILDKIT_SBOM_SCAN_CONTEXT=true` to include build context in scan
- Generate SLSA provenance with build metadata, timestamps, and inputs
- Attach attestations to image manifest

**Alternatives considered:**
- External SBOM tools (Trivy, Grype) - Additional tooling, not integrated with build
- Skip SBOM - Missing critical security feature
- Sign attestations with Cosign - Out of scope, users can add separately

**Rationale:** BuildKit native SBOM is integrated into build process. SPDX format is industry standard. Provenance provides audit trail.

### Decision 4: BuildKit daemon connection modes

Support connecting to BuildKit daemon via Unix socket (local) or TCP (remote/containerized).

**Connection options:**
- Default: `unix:///run/buildkit/buildkitd.sock`
- Docker BuildKit: `docker-container://buildkitd`
- Remote: `tcp://buildkit.example.com:1234`

**Alternatives considered:**
- Embedded BuildKit - Complex, large binary size
- Only local socket - Limits deployment options
- Only Docker integration - Not all environments use Docker

**Rationale:** Flexible connection supports local development (socket), CI/CD (Docker container), and cloud builds (remote).

### Decision 5: Package manager is always apk

Remove package manager abstraction. All packages are Wolfi apk packages. The schema specifies Wolfi package names directly.

**Package installation pattern:**
```dockerfile
RUN apk add --no-cache <packages>
```

**Common package mappings (for LLM prompt guidance):**
| Purpose | Wolfi Package |
|---------|---------------|
| Rust toolchain | `rust` |
| Node.js runtime | `nodejs-22` |
| Python runtime | `python-3.12` |
| Go toolchain | `go` |
| Java JDK | `openjdk-21` |
| Java JRE | `openjdk-21-jre` |
| SSL/TLS | `openssl` |
| Certificates | `ca-certificates` |
| Build tools | `build-base` |

**Alternatives considered:**
- Maintain apt/apk mapping - Unnecessary if only using Wolfi
- Let LLM figure out packages - Less reliable than explicit guidance

**Rationale:** Single package manager eliminates complexity. LLM prompt includes Wolfi package names for common dependencies.

### Decision 6: Direct image building with multi-app support and multiple output options

peelbox acts as a BuildKit frontend that builds images directly. Supports multi-app repositories (monorepos) with selective or batch building.

**Build workflow:**
1. Load UniversalBuild spec from JSON file (single app or array of apps)
2. Select apps to build (all by default, or specific app via `--app`)
3. For each app: Generate LLB graph internally
4. Connect to BuildKit daemon
5. Execute builds with progress streaming
6. Generate SBOM and provenance attestations per app
7. Output to selected destination (registry, Docker, tarball)

**Spec file format:**

The `--spec` file can be either:
1. **Single app** (object): When repository has single runnable application
2. **Multi-app** (array): When repository is monorepo with multiple applications

*Single-app spec (object):*
```json
{
  "version": "1.0",
  "metadata": {"project_name": "myapp", ...},
  "build": {...},
  "runtime": {...}
}
```

*Multi-app spec (array):*
```json
[
  {
    "version": "1.0",
    "metadata": {"project_name": "backend", ...},
    "build": {...},
    "runtime": {...}
  },
  {
    "version": "1.0",
    "metadata": {"project_name": "frontend", ...},
    "build": {...},
    "runtime": {...}
  }
]
```

**Note**: The `peelbox detect` command already outputs `Vec<UniversalBuild>` for monorepos, which serializes as JSON array. The `build` command consumes this same format.

**CLI interface:**
```bash
# Build all apps in spec using template with {app} placeholder
peelbox build --repo /path/to/monorepo --spec universalbuild.json --image myapp-{app}:latest

# Build specific app from multi-app spec
peelbox build --repo . --spec universalbuild.json --app backend --image backend:latest

# Single-app build (no placeholder needed)
peelbox build --repo . --spec universalbuild.json --image myapp:latest

# Build and load to Docker daemon (default)
peelbox build --repo . --spec universalbuild.json --image myapp:latest --output type=docker

# Build and save as OCI tarball
peelbox build --repo . --spec universalbuild.json --image myapp:latest --output type=oci,dest=image.tar

# Build with custom BuildKit endpoint
peelbox build --repo . --spec universalbuild.json --image myapp:latest --buildkit tcp://buildkit:1234
```

**Implementation details:**
- `--repo` specifies repository root (build context path)
- `--spec` specifies UniversalBuild JSON file (can be single object or array)
- `--image` specifies image name template (required)
  - For single-app: Use literal name like `myapp:latest`
  - For multi-app: Use template with `{app}` placeholder like `myapp-{app}:latest`
  - `{app}` is replaced with `metadata.project_name` from each UniversalBuild
- `--app` selects specific app by `metadata.project_name` (optional)
  - If spec is array and `--app` not provided, build all apps sequentially
  - If `--app` is specified, only build that app and `{app}` placeholder is replaced with the app name

**Alternatives considered:**
- LLB JSON output only - Requires additional tooling to build
- Dockerfile output only - Loses BuildKit native features
- Registry push support - Adds complexity, users can push locally built images separately
- Separate command per app - Inefficient for monorepos
- Parallel builds - Complex error handling, overwhelming progress output

**Rationale:** Multi-app support enables monorepo workflows. Sequential builds provide clear progress and error reporting. Local-only builds keep peelbox focused - users can push to registries using standard Docker/BuildKit tools after building.

### Decision 7: Distroless images with optimized 2-layer architecture

All builds produce distroless final images using a 2-stage build with optimized layer structure. No opt-out, no flag needed.

**Implementation strategy:**

Two-stage LLB build with smart layer construction:
1. **Build stage**: `wolfi-base` + build packages + build commands → artifacts
2. **Distroless stage**: `FROM scratch` + 2 layers (runtime base + app artifacts)

**Layer construction in distroless stage:**

The distroless stage uses a temporary intermediate stage to prepare the runtime base:

```
# Conceptual flow
temp-runtime: wolfi-base + apk add runtime packages → /runtime-root
distroless:   FROM scratch
              COPY --from=temp-runtime /runtime-root /     # Layer 1: Runtime base
              COPY --from=build /artifacts /app            # Layer 2: App artifacts
```

**LLB graph structure:**
```
Stage 1 (Build):
  wolfi-base → install build packages → build app → /artifacts

Stage 2 (Temp Runtime) - BuildKit internal:
  wolfi-base → install runtime packages → prepare /runtime-root → remove apk/metadata

Stage 3 (Distroless Final):
  FROM scratch
  COPY --from=temp-runtime /runtime-root /    # Layer 1: Wolfi core + runtime packages
  COPY --from=build /artifacts /app            # Layer 2: Application artifacts
```

**Key benefits:**
- **2 layers in final image**: Runtime base (layer 1) + app (layer 2)
- **Optimal caching**: Runtime deps cached separately from app code
- **No apk in final image**: Temp runtime stage removes package manager before copy
- **Minimal size**: Only runtime files + app, no package database or metadata

**Size comparison:**
- Wolfi-base runtime: ~50-100MB (includes apk, bash, package metadata)
- Distroless 2-layer: ~10-30MB (only app + runtime dependencies)

**Layer structure benefits:**
- **Layer 1 (Runtime base)**: Cached across all builds with same runtime packages
- **Layer 2 (App artifacts)**: Only rebuilt when app code changes
- **Optimal Docker caching**: Changing app code doesn't invalidate runtime layer
- **Fast rebuilds**: Most builds only rebuild layer 2

**Trade-offs:**
- **Pro**: Smallest possible image, minimal attack surface, no shell/package manager
- **Pro**: Production-ready by default (no accidental insecure deployments)
- **Pro**: Consistent behavior (no configuration decisions)
- **Pro**: Optimal layer caching (2 layers: runtime base + app)
- **Con**: Cannot install packages at runtime, no debugging tools (no shell)
- **Con**: Slightly more complex LLB graph (2 stages + temp runtime prep)

**Alternatives considered:**
- Optional flag (`--distroless`) - Users might forget, ship insecure images
- Three-stage build (Build → Runtime → Distroless) - Extra layer, larger final image
- Single-layer distroless (copy everything together) - Poor caching, rebuilds runtime deps on every app change
- Chainguard distroless images - Less flexible, don't support runtime package installation
- Manual apk deletion - Leaves package database inconsistencies

**Rationale:**
- **Security by default**: No way to accidentally ship images with package managers
- **Simplicity**: No flag to remember, no configuration decision
- **Production-ready**: All images are production-grade out of the box
- **Optimized caching**: 2-layer structure maximizes Docker layer reuse
- Temp runtime stage cleanly separates package installation from final image
- BuildKit handles layer optimization efficiently
- Matches industry best practice (Google Distroless, Chainguard images)
- For debugging: Users can `docker exec` with debug sidecar or use local dev tools

### Decision 8: Dynamic version discovery from Wolfi APKINDEX

Instead of hardcoding package versions in BuildSystem implementations, peelbox discovers available versions dynamically from the Wolfi APKINDEX at startup.

**Version discovery strategy:**

1. **Startup**: Fetch and parse APKINDEX.tar.gz (with 24-hour cache)
2. **Index available versions**: For each language/runtime, extract all available versioned packages
   - Example: `nodejs-22`, `nodejs-20`, `nodejs-18` → versions `[22, 20, 18]`
   - Example: `python-3.12`, `python-3.11`, `python-3.10` → versions `[3.12, 3.11, 3.10]`
   - Example: `openjdk-21`, `openjdk-17`, `openjdk-11` → versions `[21, 17, 11]`
3. **Select default**: Choose highest available version for each language
4. **BuildSystem query**: BuildSystems query available versions and select based on manifest

**Implementation:**

```rust
// In src/validation/wolfi_index.rs
pub struct WolfiPackageIndex {
    packages: HashSet<String>,
}

impl WolfiPackageIndex {
    /// Get all available versions for a package prefix (e.g., "nodejs" -> [22, 20, 18])
    pub fn get_versions(&self, package_prefix: &str) -> Vec<String> { ... }

    /// Get latest (highest) version for a package prefix
    pub fn get_latest_version(&self, package_prefix: &str) -> Option<String> { ... }

    /// Check if specific versioned package exists
    pub fn has_package(&self, package_name: &str) -> bool { ... }
}
```

**BuildSystem usage:**

```rust
// In build_template() method
fn build_template(&self, index: &WolfiPackageIndex, manifest_content: Option<&str>) -> BuildTemplate {
    // Parse version from manifest (e.g., package.json engines.node)
    let requested_version = parse_node_version(manifest_content);

    // Get available Node.js versions from APKINDEX
    let available_versions = index.get_versions("nodejs");

    // Select best match or fallback to latest
    let node_version = match_version(requested_version, &available_versions)
        .or_else(|| index.get_latest_version("nodejs"))
        .unwrap_or("nodejs-22".to_string());

    BuildTemplate {
        build_packages: vec![node_version.clone()],
        runtime_packages: vec![node_version],
        ...
    }
}
```

**Benefits:**
- **Auto-updates**: New Wolfi package versions automatically available (after cache refresh)
- **No hardcoding**: No need to update peelbox code when Wolfi releases new runtime versions
- **Validation**: Automatically validates that selected version exists in Wolfi
- **Flexibility**: BuildSystems can query available versions and choose best match

**Alternatives considered:**
- Hardcode versions in BuildSystem - Requires code updates for new Wolfi versions
- Always use "latest" tag - Wolfi doesn't support generic "latest" packages
- Let LLM choose version - Less deterministic, validation overhead

**Rationale:** Dynamic discovery keeps peelbox in sync with Wolfi package availability without code changes. APKINDEX is already being fetched for validation, so no additional overhead.

### Decision 9: Wolfi package validation via ValidationRule

Add Wolfi package validation to existing `Validator` system by querying package index at `https://packages.wolfi.dev/os/x86_64/APKINDEX.tar.gz`.

**Implementation:**
- New `WolfiPackageValidationRule` implementing `ValidationRule` trait
- Fetch and cache APKINDEX.tar.gz in `src/validation/wolfi_index.rs` (24-hour TTL)
- Parse APK index format (tar.gz → tar → APKINDEX file) to extract package names with versions
- Validate all packages in `build.packages` and `runtime.packages` against available package list
- Return helpful error messages with similar package name suggestions and available versions
- If user specifies version-less name (e.g., `nodejs`), suggest versioned alternatives (e.g., `nodejs-22`, `nodejs-20`)

**Integration points:**
- Validator runs on all UniversalBuild outputs in `detect` command
- Build command auto-validates before sending to BuildKit
- BuildSystem implementations validate their package lists on init (fail fast)

**Validation examples:**
```rust
// Valid
build.packages = ["nodejs-22", "build-base"]  // ✓
runtime.packages = ["nodejs-22"]              // ✓

// Invalid - no generic nodejs package in Wolfi
build.packages = ["nodejs"]                   // ✗ Error: "nodejs not found. Did you mean: nodejs-22, nodejs-20, nodejs-18?"

// Invalid - typo
runtime.packages = ["pythonn-3.12"]           // ✗ Error: "pythonn-3.12 not found. Did you mean: python-3.12?"
```

**Alternatives considered:**
- LLM tools for validation - Architecture no longer uses tool system
- No validation (trust deterministic code) - Risk of typos in Wolfi package names
- External API - No public search API exists for Wolfi
- Hardcoded package list - Becomes stale quickly
- BuildKit-time validation - Fails late, wastes build time

**Rationale:** Early validation catches package name errors before expensive BuildKit builds. Version-aware validation prevents common mistake of using generic package names. Integrates with existing validation system rather than adding LLM tool complexity.

### Decision 10: Minimum BuildKit version v0.11.0

Require [BuildKit v0.11.0](https://github.com/moby/buildkit/releases/tag/v0.11.0) or later for SBOM and provenance attestation support.

**Version check:**
- Query BuildKit daemon version on connection
- Fail with clear error message if version < 0.11.0
- Document version requirement in README and error messages

**Compatibility notes:**
- Docker Desktop 4.17+ includes BuildKit 0.11+
- Docker Engine 23.0+ includes BuildKit 0.11+
- Standalone buildkitd must be 0.11.0+

**Alternatives considered:**
- Support older versions without attestations - Fragments feature set
- Higher minimum (0.12, 0.13) - Unnecessarily restrictive
- No version check - Confusing errors when attestations fail

**Rationale:** v0.11.0 is the first version with SBOM/provenance support. It's widely available in current Docker releases. Clear version requirement prevents confusing failures.

## Risks / Trade-offs

| Risk                              | Mitigation                                                               |
|-----------------------------------|--------------------------------------------------------------------------|
| Wolfi package availability        | Package validation tool; document alternatives; version-aware validation |
| `buildkit-client` crate stability | Crate actively maintained; fallback to `buildctl` CLI if critical issues |
| BuildKit daemon requirement       | Clear error messages; document setup for Docker Desktop, standalone, CI  |
| SBOM scan time                    | BuildKit's Syft scanner is optimized; mandatory for security compliance  |
| Wolfi package name differences    | Enhanced validation with fuzzy matching; explicit guidance in prompts    |
| BuildKit version mismatch         | Version check on connection with clear error message                     |

## Migration Plan

**Note**: This is a breaking change. No backwards compatibility maintained (peelbox is pre-1.0).

### Phase 1: Schema Breaking Changes
1. Remove `build.base` and `runtime.base` from UniversalBuild schema
2. Remove `build_image` and `runtime_image` from BuildTemplate
3. Update all 16 BuildSystem implementations to return Wolfi packages
4. Update all test fixtures (version stays `1.0`)

### Phase 2: Wolfi Package Validation
1. Implement APKINDEX fetcher and parser (`src/validation/wolfi_index.rs`)
2. Implement `WolfiPackageValidationRule` validation rule
3. Integrate into existing Validator

### Phase 3: BuildKit Client Integration
1. Add `buildkit-client` dependency
2. Implement `src/buildkit/client.rs` with connection management
3. Implement `src/buildkit/llb.rs` for 2-stage distroless LLB with optimized layers (always enabled)
4. Implement `src/buildkit/progress.rs` for build progress display

### Phase 4: SBOM and Build Command
1. Add SBOM and provenance attestation generation
2. Implement `peelbox build` CLI command (distroless mandatory, no flag)
3. Remove Dockerfile generation entirely (`src/output/dockerfile.rs`)

### Rollback Strategy
**None**. This is an all-or-nothing breaking change. Users on older versions must use older peelbox versions if they need Dockerfile output.
