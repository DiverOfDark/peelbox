# SBOM and Provenance Attestations

This document describes how to generate Software Bill of Materials (SBOM) and SLSA provenance attestations when building container images with aipack and BuildKit.

## Overview

BuildKit v0.11.0+ includes native support for generating:
- **SBOM (Software Bill of Materials)** - Complete inventory of software components
- **SLSA Provenance** - Build metadata and attestations for supply chain security

These attestations are attached to the image manifest and can be inspected using tools like `docker buildx imagetools inspect`.

## SBOM Attestations

### What is SBOM?

A Software Bill of Materials (SBOM) is a complete inventory of all software components, libraries, and dependencies in a container image. aipack generates SBOM attestations in SPDX format using BuildKit's built-in Syft scanner.

### Generating SBOM

Use buildctl with `--output` flags to generate SBOM attestations:

```bash
# Generate LLB with aipack frontend
aipack frontend --spec universalbuild.json > build.llb

# Build with SBOM attestation
buildctl build \
  --local context=. \
  --output type=image,name=myapp:latest,push=false \
  --opt attest:sbom= \
  < build.llb
```

### SBOM Configuration Options

Enable context scanning to include build context files in SBOM:

```bash
buildctl build \
  --local context=. \
  --output type=image,name=myapp:latest,push=false \
  --opt attest:sbom= \
  --opt build-arg:BUILDKIT_SBOM_SCAN_CONTEXT=true \
  < build.llb
```

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
- Build timestamp
- Builder information
- Source repository
- Build inputs and parameters
- Reproducibility metadata

### Generating Provenance

Generate SLSA provenance attestations:

```bash
# Build with provenance attestation
buildctl build \
  --local context=. \
  --output type=image,name=myapp:latest,push=false \
  --opt attest:provenance=mode=max \
  < build.llb
```

### Provenance Modes

BuildKit supports different provenance modes:

| Mode | Description | Use Case |
|------|-------------|----------|
| `min` | Minimal provenance | Fast builds, basic metadata |
| `max` | Full provenance | Production builds, complete audit trail |

```bash
# Minimal provenance
--opt attest:provenance=mode=min

# Maximum provenance (recommended for production)
--opt attest:provenance=mode=max
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

Generate both SBOM and provenance attestations:

```bash
buildctl build \
  --local context=. \
  --output type=image,name=myapp:latest,push=false \
  --opt attest:sbom= \
  --opt attest:provenance=mode=max \
  --opt build-arg:BUILDKIT_SBOM_SCAN_CONTEXT=true \
  < build.llb
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
        uses: docker/setup-buildx-action@v3

      - name: Install aipack
        run: |
          curl -L https://github.com/yourusername/aipack/releases/latest/download/aipack-linux-amd64 -o aipack
          chmod +x aipack

      - name: Generate build spec
        run: ./aipack detect . > universalbuild.json

      - name: Generate LLB
        run: ./aipack frontend --spec universalbuild.json > build.llb

      - name: Build with attestations
        run: |
          buildctl build \
            --local context=. \
            --output type=image,name=ghcr.io/${{ github.repository }}:${{ github.sha }},push=true \
            --opt attest:sbom= \
            --opt attest:provenance=mode=max \
            --opt build-arg:BUILDKIT_SBOM_SCAN_CONTEXT=true \
            < build.llb
```

### GitLab CI

```yaml
build-image:
  stage: build
  image: moby/buildkit:latest
  services:
    - docker:dind
  script:
    - aipack detect . > universalbuild.json
    - aipack frontend --spec universalbuild.json > build.llb
    - |
      buildctl build \
        --local context=. \
        --output type=image,name=$CI_REGISTRY_IMAGE:$CI_COMMIT_SHA,push=true \
        --opt attest:sbom= \
        --opt attest:provenance=mode=max \
        < build.llb
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

1. **Always generate SBOM** - Track all dependencies for vulnerability scanning
2. **Use provenance mode=max** - Maximum metadata for audit trails
3. **Sign attestations** - Use Cosign to cryptographically sign SBOM/provenance
4. **Scan regularly** - Use tools like Grype or Trivy to scan SBOM for CVEs
5. **Store attestations** - Archive SBOM/provenance for compliance and auditing

## Further Reading

- [BuildKit Attestations Documentation](https://docs.docker.com/build/attestations/)
- [SBOM Guide](https://docs.docker.com/build/metadata/attestations/sbom/)
- [SLSA Provenance Specification](https://slsa.dev/provenance/)
- [in-toto Attestation Framework](https://in-toto.io/)
- [Cosign Signing Tool](https://docs.sigstore.dev/cosign/overview/)
