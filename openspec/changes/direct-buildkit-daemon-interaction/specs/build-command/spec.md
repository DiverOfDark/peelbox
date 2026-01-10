# build-command Specification Delta

## Purpose
Define the `peelbox build` command as the primary interface for building container images, replacing the `peelbox frontend | buildctl` workflow with a single integrated command.

## ADDED Requirements

### Requirement: Build Command Interface
The system SHALL provide a `peelbox build` command that detects, builds, and outputs container images in a single operation.

#### Scenario: Build from spec file
- **WHEN** `peelbox build --spec universalbuild.json --tag app:latest` is executed
- **THEN** the system loads the UniversalBuild spec from file
- **AND** connects to BuildKit daemon
- **AND** builds the image with specified tag
- **AND** loads image into Docker daemon (default output)

#### Scenario: Missing spec argument
- **WHEN** `peelbox build --tag app:latest` is executed (no --spec flag)
- **THEN** the system returns an error requiring `--spec` to be specified
- **AND** provides example usage with spec file

#### Scenario: Build with service selection (monorepo)
- **WHEN** `peelbox build --spec spec.json --service api --tag api:latest` is executed
- **AND** spec contains multiple services
- **THEN** the system builds only the specified service
- **AND** uses that service's build and runtime configuration

#### Scenario: Missing tag argument
- **WHEN** `peelbox build` is executed without `--tag` flag
- **THEN** the system returns an error requiring `--tag` to be specified
- **AND** provides example usage

---

### Requirement: Output Type Control
The system SHALL support multiple output types for built images.

#### Scenario: Docker daemon output (default)
- **WHEN** `peelbox build --tag app:latest` is executed
- **AND** no `--output` flag is specified
- **THEN** the image is built and loaded into local Docker daemon
- **AND** image is immediately available to `docker run`

#### Scenario: OCI tarball export
- **WHEN** `peelbox build --spec spec.json --tag app:latest --output type=oci,dest=app.tar` is executed
- **THEN** the image is built and exported as OCI layout tarball
- **AND** tarball is saved to specified file path
- **AND** tarball includes SBOM and provenance attestations

---

### Requirement: Platform Targeting
The system SHALL support building images for specific platforms or multi-platform builds.

#### Scenario: Single platform target
- **WHEN** `peelbox build --tag app:latest --platform linux/amd64` is executed
- **THEN** the image is built exclusively for linux/amd64
- **AND** SBOM reflects the target platform

#### Scenario: Multi-platform build
- **WHEN** `peelbox build --tag app:latest --platform linux/amd64,linux/arm64` is executed
- **THEN** BuildKit builds both platform variants
- **AND** creates a multi-platform manifest
- **AND** SBOM includes both platform architectures

#### Scenario: No platform specified
- **WHEN** no `--platform` flag is provided
- **THEN** the system builds for the current host platform

---

### Requirement: Entrypoint Override
The system SHALL allow overriding the entrypoint at build time without modifying the spec.

#### Scenario: Override entrypoint
- **WHEN** `peelbox build --tag app:latest --entrypoint /bin/sh` is executed
- **THEN** the built image uses `/bin/sh` as entrypoint
- **AND** the original spec's entrypoint is ignored

#### Scenario: Override with arguments
- **WHEN** `peelbox build --tag app:latest --entrypoint "/app/myapp --verbose"` is executed
- **THEN** the entrypoint includes both binary and arguments

---

### Requirement: Build Progress Display
The system SHALL display real-time build progress with layer status and logs.

#### Scenario: Spec file required
- **WHEN** `peelbox build --tag app:latest` is executed without `--spec`
- **THEN** the system returns an error
- **AND** error message states `--spec` flag is required
- **AND** suggests running `peelbox detect . > universalbuild.json` first

#### Scenario: Default progress display
- **WHEN** a build is running in an interactive terminal
- **THEN** progress bars show current operations
- **AND** layer build status is displayed (building, cached, complete)
- **AND** build logs are streamed in real-time

#### Scenario: Non-TTY output
- **WHEN** running in a non-interactive environment (CI/CD)
- **THEN** progress is displayed as plain text log lines
- **AND** no ANSI escape codes or progress bars are used

#### Scenario: Quiet mode
- **WHEN** `--quiet` flag is used
- **THEN** progress bars and streaming logs are suppressed
- **AND** only final summary and errors are shown

#### Scenario: Verbose mode
- **WHEN** `--verbose` flag is used
- **THEN** full BuildKit operation logs are shown
- **AND** includes internal BuildKit state transitions

---

### Requirement: Build Summary Output
The system SHALL display a build summary upon completion.

#### Scenario: Successful build summary
- **WHEN** a build completes successfully
- **THEN** the system displays:
  - Final image size
  - Build duration
  - Cache hit ratio (cached layers / total layers)
  - SBOM generation confirmation
  - Provenance generation confirmation
  - Output location (Docker daemon, registry, file path)

#### Scenario: Failed build summary
- **WHEN** a build fails
- **THEN** the system displays:
  - Failing layer and command
  - Error message from BuildKit
  - Build duration until failure
  - Suggests next steps (check logs, fix Dockerfile equivalent)

---

### Requirement: BuildKit Daemon Override
The system SHALL allow explicit BuildKit daemon address specification.

#### Scenario: Override daemon address
- **WHEN** `peelbox build --buildkit tcp://remote.example.com:1234 --tag app:latest` is executed
- **THEN** the system connects only to the specified daemon
- **AND** auto-detection is skipped

#### Scenario: Invalid daemon address
- **WHEN** `--buildkit` specifies an unreachable address
- **THEN** the system returns connection error
- **AND** does not fall back to auto-detection

## REMOVED Requirements

### ~~Requirement: Frontend Command~~
**REASON:** `peelbox frontend` command removed entirely - replaced by `peelbox build`.

### ~~Requirement: Registry Push Support~~
**REASON:** Deferred to future version (v0.5.0+) to reduce initial scope complexity.

### ~~Requirement: Buildctl Dependency~~
**REASON:** Direct gRPC client eliminates need for external `buildctl` CLI tool.

### ~~Requirement: LLB Stdout Protocol~~
**REASON:** LLB is submitted via gRPC session, not written to stdout for piping to buildctl.

### ~~Requirement: Auto-detection in Build Command~~
**REASON:** Build command requires `--spec` flag - detection is separate step (`peelbox detect`).
