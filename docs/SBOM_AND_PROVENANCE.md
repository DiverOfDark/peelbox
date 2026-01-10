# SBOM and Provenance Attestations

This document describes how to generate Software Bill of Materials (SBOM) and SLSA provenance attestations when building container images with peelbox and BuildKit.

## Overview

BuildKit v0.11.0+ includes native support for generating:
- **SBOM (Software Bill of Materials)** - Complete inventory of software components in SPDX format
- **SLSA Provenance** - Build metadata and attestations for supply chain security

peelbox **automatically enables** both SBOM and SLSA provenance attestations by default for all builds. Attestations are embedded in the OCI image manifest and can be inspected using tools like `docker buildx imagetools inspect`.

## Default Behavior

By default, peelbox generates:
- **SBOM attestation** in SPDX-JSON format using BuildKit's built-in Syft scanner
- **SLSA provenance** in maximum mode (mode=max) with complete build metadata
- **Build context scanning** to include source files in SBOM

All builds automatically include these attestations unless explicitly disabled.

## Using peelbox build Command

peelbox provides a high-level `build` command that handles attestation generation automatically.

### Basic Usage (Attestations Enabled by Default)

```bash
# Build with automatic SBOM and provenance (default behavior)
peelbox build --spec universalbuild.json --tag myapp:latest

# Output:
# INFO SBOM attestation enabled (SPDX format)
# INFO SLSA provenance attestation enabled (mode: Max)
```

### Controlling Attestations

```bash
# Disable SBOM attestation
peelbox build --spec spec.json --tag app:latest --no-sbom

# Disable provenance attestation
peelbox build --spec spec.json --tag app:latest --no-provenance

# Disable both
peelbox build --spec spec.json --tag app:latest --no-sbom --no-provenance

# Use minimal provenance mode (faster builds)
peelbox build --spec spec.json --tag app:latest --provenance min

# Use maximum provenance mode (complete audit trail, default)
peelbox build --spec spec.json --tag app:latest --provenance max
```

## SBOM Attestations

### What is SBOM?

A Software Bill of Materials (SBOM) is a complete inventory of all software components, libraries, and dependencies in a container image. peelbox generates SBOM attestations in SPDX-JSON format using BuildKit's built-in Syft scanner.

### Automatic Generation

SBOM is generated automatically during builds and includes:
- All runtime packages (from Wolfi)
- Application dependencies
- Build context files (when scan_context is enabled)

No manual configuration required - peelbox handles this transparently.

### Viewing SBOM

Inspect the SBOM attached to the image:

```bash
# View all attestations
docker buildx imagetools inspect myapp:latest --format '{{json .SBOM}}'

# Extract SBOM to file
docker buildx imagetools inspect myapp:latest \
  --format '{{json .SBOM}}' > sbom.json
```

## SLSA Provenance Attestations

### What is SLSA Provenance?

SLSA (Supply-chain Levels for Software Artifacts) provenance provides build metadata including:
- Build timestamp and duration
- Builder information (BuildKit version)
- Build inputs and parameters
- LLB definition digest
- Reproducibility metadata

### Automatic Generation

SLSA provenance is generated automatically during builds in **maximum mode** by default, providing:
- Complete build metadata
- Full audit trail
- Supply chain security compliance

### Provenance Modes

peelbox supports two provenance modes:

| Mode | Description | Use Case | CLI Flag |
|------|-------------|----------|----------|
| `min` | Minimal provenance | Fast builds, basic metadata | `--provenance min` |
| `max` | Full provenance (default) | Production builds, complete audit trail | `--provenance max` |

```bash
# Maximum provenance (default, recommended for production)
peelbox build --spec spec.json --tag app:latest

# Minimal provenance (faster builds)
peelbox build --spec spec.json --tag app:latest --provenance min
```

### Viewing Provenance

Inspect provenance metadata:

```bash
# View provenance
docker buildx imagetools inspect myapp:latest \
  --format '{{json .Provenance}}'

# Extract to file
docker buildx imagetools inspect myapp:latest \
  --format '{{json .Provenance}}' > provenance.json
```

## Combined SBOM and Provenance

Both SBOM and provenance attestations are enabled by default:

```bash
# Automatic (both enabled by default)
peelbox build --spec universalbuild.json --tag myapp:latest

# Explicitly configure both
peelbox build --spec spec.json --tag app:latest --provenance max
```

## Programmatic API

For advanced use cases, you can configure attestations programmatically using the Rust API:

```rust
use peelbox::buildkit::{AttestationConfig, BuildSession, ProvenanceMode};

// Custom attestation configuration
let attestation_config = AttestationConfig {
    sbom: true,
    provenance: Some(ProvenanceMode::Max),
    scan_context: true,
};

// Create session with custom attestations
let session = BuildSession::new(connection, context_path, output_path, image_tag)
    .with_attestations(attestation_config);
```

### Default Configuration

```rust
impl Default for AttestationConfig {
    fn default() -> Self {
        Self {
            sbom: true,                          // SBOM enabled
            provenance: Some(ProvenanceMode::Max), // Maximum provenance
            scan_context: true,                   // Scan build context
        }
    }
}
```

## Signing Attestations with Cosign

After generating attestations, you can sign them with Cosign for cryptographic verification:

```bash
# Sign the image and its attestations
cosign sign myapp:latest

# Verify signatures
cosign verify myapp:latest
```

## CI/CD Integration Examples

### GitHub Actions

```yaml
name: Build and Attest
on: push

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Set up BuildKit
        run: |
          docker run -d --rm --name buildkitd --privileged \
            -p 127.0.0.1:1234:1234 \
            moby/buildkit:latest --addr tcp://0.0.0.0:1234

      - name: Install peelbox
        run: |
          curl -L https://github.com/yourusername/peelbox/releases/latest/download/peelbox-linux-amd64 -o peelbox
          chmod +x peelbox

      - name: Build with attestations (automatic)
        run: |
          ./peelbox detect . > universalbuild.json
          ./peelbox build --spec universalbuild.json \
            --tag ghcr.io/${{ github.repository }}:${{ github.sha }} \
            --buildkit tcp://127.0.0.1:1234

      - name: Verify attestations
        run: |
          docker load < ghcr.io-${{ github.repository }}-${{ github.sha }}.tar
          docker buildx imagetools inspect ghcr.io/${{ github.repository }}:${{ github.sha }}
```

### GitLab CI

```yaml
build-image:
  stage: build
  image: rust:latest
  services:
    - docker:dind
  script:
    # Install peelbox
    - cargo install --git https://github.com/yourusername/peelbox peelbox

    # Build with automatic attestations
    - peelbox detect . > universalbuild.json
    - peelbox build --spec universalbuild.json \
        --tag $CI_REGISTRY_IMAGE:$CI_COMMIT_SHA \
        --buildkit tcp://docker:2376
```

## Attestation Format

Attestations follow the [in-toto](https://in-toto.io/) specification and are stored as image layers in the OCI image manifest.

### SBOM Format (SPDX)

```json
{
  "spdxVersion": "SPDX-2.3",
  "dataLicense": "CC0-1.0",
  "SPDXID": "SPDXRef-DOCUMENT",
  "name": "myapp:latest",
  "packages": [
    {
      "SPDXID": "SPDXRef-Package-rust",
      "name": "rust",
      "versionInfo": "1.75",
      "filesAnalyzed": false
    }
  ]
}
```

### Provenance Format (SLSA)

```json
{
  "buildType": "https://mobyproject.org/buildkit@v1",
  "builder": {
    "id": "https://github.com/moby/buildkit"
  },
  "metadata": {
    "buildInvocationId": "...",
    "buildStartedOn": "2024-01-01T00:00:00Z",
    "buildFinishedOn": "2024-01-01T00:05:00Z"
  },
  "materials": [
    {
      "uri": "pkg:docker/cgr.dev/chainguard/wolfi-base@latest"
    }
  ]
}
```

## Requirements

- BuildKit v0.11.0 or later
- Docker BuildKit enabled (Docker Desktop 4.17+, Docker Engine 23.0+)
- buildctl CLI tool installed

## Security Best Practices

1. **Default is secure** - peelbox automatically generates SBOM and provenance with maximum metadata
2. **Sign attestations** - Use Cosign to cryptographically sign SBOM/provenance for verification
3. **Scan regularly** - Use tools like Grype or Trivy to scan SBOM for CVEs
4. **Store attestations** - Archive SBOM/provenance for compliance and auditing
5. **Disable only when necessary** - Only use `--no-sbom` or `--no-provenance` if you have specific requirements

## Quick Reference

### CLI Flags

| Flag | Effect | Default |
|------|--------|---------|
| (none) | SBOM + provenance (max) enabled | ✓ |
| `--no-sbom` | Disable SBOM | - |
| `--no-provenance` | Disable provenance | - |
| `--provenance min` | Minimal provenance metadata | - |
| `--provenance max` | Full provenance metadata | ✓ |

### Example Commands

```bash
# Default (recommended) - Full attestations
peelbox build --spec spec.json --tag app:latest

# Fast builds - Minimal provenance
peelbox build --spec spec.json --tag app:latest --provenance min

# No attestations (not recommended)
peelbox build --spec spec.json --tag app:latest --no-sbom --no-provenance

# Verify attestations after build
docker buildx imagetools inspect app:latest --format '{{json .SBOM}}'
docker buildx imagetools inspect app:latest --format '{{json .Provenance}}'
```

## Troubleshooting

**Q: My builds are slower after enabling attestations**
A: Use `--provenance min` for faster builds, or `--no-sbom --no-provenance` to disable (not recommended for production).

**Q: How do I view attestations in the built image?**
A: Load the tar into Docker and use `docker buildx imagetools inspect <image>` to view attestations.

**Q: Are attestations signed?**
A: BuildKit generates unsigned attestations. Use Cosign to add cryptographic signatures after the build.

**Q: Can I customize SBOM scanner settings?**
A: BuildKit uses built-in Syft scanner. peelbox enables context scanning by default via `scan_context: true`.

## Requirements

- BuildKit v0.11.0 or later
- Docker BuildKit enabled (Docker Desktop 4.17+, Docker Engine 23.0+)
- For verification: `docker buildx` CLI plugin

## Further Reading

- [BuildKit Attestations Documentation](https://docs.docker.com/build/attestations/)
- [SBOM Guide](https://docs.docker.com/build/metadata/attestations/sbom/)
- [SLSA Provenance Specification](https://slsa.dev/provenance/)
- [in-toto Attestation Framework](https://in-toto.io/)
- [Cosign Signing Tool](https://docs.sigstore.dev/cosign/overview/)
