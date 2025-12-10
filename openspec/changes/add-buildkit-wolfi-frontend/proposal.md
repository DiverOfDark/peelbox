# Change: Add BuildKit Frontend with Wolfi Base Images

## Why

aipack currently generates Dockerfiles that require external tooling to build images. This approach has limitations:

1. **Security**: Traditional base images contain unnecessary packages, increasing attack surface and CVE exposure
2. **Supply Chain**: No built-in SBOM generation or provenance tracking
3. **Integration**: Dockerfile output requires additional build tooling; no direct BuildKit integration
4. **Image Size**: Debian-based images are significantly larger than necessary

Wolfi/Chainguard images are purpose-built for containers with minimal attack surface, glibc compatibility, and daily security updates.

## What Changes

1. **BuildKit Frontend** - Native BuildKit integration via gRPC using `buildkit-client` crate
   - Direct daemon connection (Unix socket, TCP, Docker container)
   - LLB graph generation internally
   - Real-time build progress streaming
   - Registry push and tarball export support
   - Requires BuildKit v0.11.0+ (Docker Desktop 4.17+ / Docker Engine 23.0+)

2. **Wolfi Base Images** - Single `cgr.dev/chainguard/wolfi-base` for all builds
   - Language toolchains installed via apk (rust, nodejs, python, go, java)
   - Minimal runtime packages for final image
   - No apt-get, no traditional distro images

3. **Wolfi Package Tools** - LLM tools for package validation and discovery
   - `validate_wolfi_packages` - Check package names exist in Wolfi repository
   - `search_wolfi_packages` - Search for packages by keyword
   - Fetches and caches APKINDEX from `packages.wolfi.dev`

4. **SBOM and Provenance** - Supply chain security attestations
   - SPDX format SBOM via BuildKit's Syft scanner
   - SLSA provenance with build metadata
   - Attached to image manifest

5. **New `build` Command** - Direct image building
   - `aipack build --tag image:tag --push`
   - `--output type=docker|oci|tar` for export options
   - `--buildkit <endpoint>` for daemon configuration

6. **Breaking Changes**
   - Remove Dockerfile generation (`--format dockerfile`)
   - Remove apt-get/package manager abstraction (Wolfi apk only)

## Impact

- **Affected specs:** buildkit-frontend (new), wolfi-images (new), wolfi-package-tools (new), output-formats (new)
- **Affected code:**
  - `src/buildkit/` - New module for BuildKit client, LLB generation, progress display
  - `src/detection/tools/wolfi_packages.rs` - New package validation/search tools
  - `src/cli/commands.rs` - New `build` command
  - `src/output/dockerfile.rs` - **REMOVED**
  - `src/output/schema.rs` - Simplified (apk only)
  - `src/detection/tools/best_practices.rs` - Wolfi package templates
  - `Cargo.toml` - Add `buildkit-client` dependency
- **External API:** New `build` command, remove Dockerfile output
- **Breaking changes:** Dockerfile generation removed, Wolfi-only images, BuildKit v0.11.0+ required
