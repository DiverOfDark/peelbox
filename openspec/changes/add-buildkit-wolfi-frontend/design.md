## Context

aipack generates container build specifications from repository analysis. This design document covers implementing a native BuildKit frontend that connects to the BuildKit daemon, generates secure container images using Wolfi base images, and produces SBOM attestations.

**Stakeholders:**
- Developers using aipack to containerize applications
- Security teams requiring minimal, auditable container images with SBOM
- Platform teams integrating aipack into CI/CD pipelines

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

### Decision 2: Single wolfi-base image for all builds

Use `cgr.dev/chainguard/wolfi-base` as the base image for both build and runtime stages. Language toolchains and runtime dependencies are installed via apk.

**Image strategy:**
- Build stage: `wolfi-base` + language toolchain packages (e.g., `rust`, `nodejs`, `python-3`)
- Runtime stage: `wolfi-base` + minimal runtime packages (e.g., `glibc`, `ca-certificates`)

**Alternatives considered:**
- Language-specific Chainguard images (`cgr.dev/chainguard/rust`) - Less flexible, harder to customize
- Distroless runtime images - Cannot install runtime dependencies if needed
- Multiple base image options - Unnecessary complexity

**Rationale:** Single base image simplifies the system. All dependencies managed uniformly via apk. Wolfi packages are optimized for containers and receive daily security updates.

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

### Decision 6: Direct image building with multiple output options

aipack acts as a BuildKit frontend that builds images directly. Supports pushing to registry, exporting to Docker, or saving as local tarball.

**Build workflow:**
1. Detect repository build requirements
2. Generate LLB graph internally
3. Connect to BuildKit daemon
4. Execute build with progress streaming
5. Generate SBOM and provenance attestations
6. Output to selected destination (registry, Docker, tarball)

**CLI interface:**
```bash
# Build and push to registry
aipack build --tag myregistry/myapp:latest --push

# Build and load to Docker daemon
aipack build --tag myapp:latest --output type=docker

# Build and save as OCI tarball
aipack build --tag myapp:latest --output type=oci,dest=image.tar

# Build and save as Docker tarball
aipack build --tag myapp:latest --output type=tar,dest=image.tar

# Build with custom BuildKit endpoint
aipack build --tag myapp:latest --buildkit tcp://buildkit:1234
```

**Alternatives considered:**
- LLB JSON output only - Requires additional tooling to build
- Dockerfile output only - Loses BuildKit native features
- Registry push only - Limits local development workflows

**Rationale:** Multiple output options support all workflows: CI/CD (push), local development (Docker), air-gapped environments (tarball).

### Decision 7: Wolfi package validation and discovery tools

Add LLM tools for validating and discovering Wolfi packages by querying the package index at `https://packages.wolfi.dev/os/x86_64/APKINDEX.tar.gz`.

**New LLM tools:**
1. `validate_wolfi_packages` - Check if package names exist in Wolfi repository
2. `search_wolfi_packages` - Search for packages by keyword or description

**Implementation:**
- Fetch and cache APKINDEX.tar.gz (refresh periodically)
- Parse APK index format to extract package names and descriptions
- Tools return validation results or search matches

**Alternatives considered:**
- No validation (trust LLM) - Higher risk of invalid package names
- External API - No public search API exists for Wolfi
- Hardcoded package list - Becomes stale quickly

**Rationale:** Real-time validation against package index ensures accurate builds. Search tool helps LLM find correct package names when unsure.

### Decision 8: Minimum BuildKit version v0.11.0

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

| Risk | Mitigation |
|------|------------|
| Wolfi package availability | Package validation tool; LLM search tool; document alternatives |
| `buildkit-client` crate stability | Crate actively maintained; fallback to `buildctl` CLI if critical issues |
| BuildKit daemon requirement | Clear error messages; document setup for Docker Desktop, standalone, CI |
| SBOM scan time | Make attestations optional via `--no-sbom` flag |
| Wolfi package name differences | LLM tools for validation and search; explicit guidance in prompt |
| BuildKit version mismatch | Version check on connection with clear error message |

## Migration Plan

1. **Phase 1**: Implement BuildKit client integration with wolfi-base builds
2. **Phase 2**: Add Wolfi package validation and search tools
3. **Phase 3**: Add SBOM and provenance attestation generation
4. **Phase 4**: Remove Dockerfile generation (breaking change)
5. **Rollback**: Keep Dockerfile generator until BuildKit frontend is stable
