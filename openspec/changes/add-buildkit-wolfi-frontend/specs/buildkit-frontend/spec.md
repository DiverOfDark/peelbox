## ADDED Requirements

### Requirement: BuildKit Daemon Connection
The system SHALL connect to a BuildKit daemon via gRPC to execute container builds.

#### Scenario: Connect via Unix socket
- **WHEN** no `--buildkit` flag is specified
- **THEN** the system connects to `unix:///run/buildkit/buildkitd.sock`
- **AND** returns a clear error if the daemon is not running

#### Scenario: Connect via TCP
- **WHEN** `--buildkit tcp://host:port` is specified
- **THEN** the system connects to the specified TCP endpoint
- **AND** supports TLS if the endpoint requires it

#### Scenario: Connect via Docker container
- **WHEN** `--buildkit docker-container://buildkitd` is specified
- **THEN** the system connects through Docker's BuildKit integration

---

### Requirement: BuildKit Version Validation
The system SHALL validate that the connected BuildKit daemon meets minimum version requirements.

#### Scenario: Valid BuildKit version
- **WHEN** connecting to BuildKit daemon version 0.11.0 or later
- **THEN** the connection succeeds and build proceeds normally

#### Scenario: Invalid BuildKit version
- **WHEN** connecting to BuildKit daemon version older than 0.11.0
- **THEN** the system returns a clear error message
- **AND** the error explains that v0.11.0+ is required for SBOM/provenance support
- **AND** suggests upgrading Docker Desktop to 4.17+ or Docker Engine to 23.0+

---

### Requirement: LLB Graph Generation
The system SHALL generate BuildKit LLB graphs from UniversalBuild specifications using wolfi-base.

#### Scenario: Generate LLB for any language
- **WHEN** a UniversalBuild specification is provided
- **THEN** the system generates an LLB graph with build stage using `cgr.dev/chainguard/wolfi-base`
- **AND** installs language toolchain via `apk add`
- **AND** runtime stage uses `cgr.dev/chainguard/wolfi-base` with minimal runtime packages

#### Scenario: Generate LLB with cache mounts
- **WHEN** a UniversalBuild specifies cache paths in build.cache
- **THEN** the LLB graph includes cache mount operations for each path
- **AND** cache mounts use `sharing=shared` mode for concurrent builds

#### Scenario: Generate LLB with multi-stage build
- **WHEN** a UniversalBuild specifies separate build and runtime stages
- **THEN** the LLB graph creates two distinct stage definitions
- **AND** artifacts are copied from build stage to runtime stage

---

### Requirement: Direct Image Building
The system SHALL build container images directly by sending LLB to the BuildKit daemon.

#### Scenario: Build and push to registry
- **WHEN** `aipack build --tag registry/image:tag --push` is executed
- **THEN** the system builds the image and pushes to the specified registry
- **AND** displays build progress in real-time

#### Scenario: Build and export to Docker
- **WHEN** `aipack build --tag image:tag --output type=docker` is executed
- **THEN** the system builds the image and loads it into local Docker daemon

#### Scenario: Build progress streaming
- **WHEN** a build is in progress
- **THEN** the system streams build logs and progress to stdout
- **AND** shows layer caching status (cached vs rebuilt)

---

### Requirement: SBOM Attestation Generation
The system SHALL generate SBOM attestations in SPDX format for all builds.

#### Scenario: Generate SBOM by default
- **WHEN** a build completes successfully
- **THEN** an SBOM attestation is generated using BuildKit's Syft scanner
- **AND** the SBOM is attached to the image manifest in SPDX JSON format

#### Scenario: Include build context in SBOM
- **WHEN** SBOM is generated
- **THEN** the scan includes files from the build context
- **AND** the scan includes packages installed in all build stages

#### Scenario: Disable SBOM generation
- **WHEN** `--no-sbom` flag is specified
- **THEN** no SBOM attestation is generated
- **AND** build completes faster without scanning overhead

---

### Requirement: SLSA Provenance Attestation
The system SHALL generate SLSA provenance attestations for build transparency.

#### Scenario: Generate provenance by default
- **WHEN** a build completes successfully
- **THEN** a SLSA provenance attestation is generated
- **AND** includes build timestamps, source references, and build inputs

#### Scenario: Provenance metadata
- **WHEN** provenance is generated
- **THEN** it includes aipack version as the build tool
- **AND** includes the detected language and build system
- **AND** includes repository URL if available from git

---

### Requirement: BuildKit Cache Optimization
The system SHALL generate LLB graphs that leverage BuildKit's caching capabilities.

#### Scenario: Layer ordering for cache efficiency
- **WHEN** an LLB graph is generated
- **THEN** package installation precedes source code copying
- **AND** dependency lock files are copied before full source

#### Scenario: Deterministic cache keys
- **WHEN** cache mounts are generated
- **THEN** cache IDs are deterministic based on project name and path
- **AND** repeated builds reuse the same cache
