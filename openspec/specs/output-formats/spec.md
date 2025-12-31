# output-formats Specification

## Purpose
TBD - created by archiving change add-buildkit-wolfi-frontend. Update Purpose after archive.
## Requirements
### Requirement: Build Command
The system SHALL provide a `build` command that builds container images via BuildKit.

#### Scenario: Build single app with image name
- **WHEN** `peelbox build --repo /path/to/repo --spec spec.json --image myapp:latest` is executed
- **THEN** the system loads the spec, generates LLB, and builds the image
- **AND** the image is named `myapp:latest`

#### Scenario: Build multi-app with template
- **WHEN** `peelbox build --repo /path/to/repo --spec spec.json --image myapp-{app}:latest` is executed
- **AND** spec contains apps named "backend" and "frontend"
- **THEN** the system builds both apps sequentially
- **AND** produces images `myapp-backend:latest` and `myapp-frontend:latest`

#### Scenario: Build specific app from multi-app spec
- **WHEN** `peelbox build --repo /path/to/repo --spec spec.json --app backend --image backend:latest` is executed
- **AND** spec contains multiple apps including "backend"
- **THEN** the system builds only the "backend" app
- **AND** produces image `backend:latest`

#### Scenario: Export to Docker daemon
- **WHEN** `--output type=docker` is specified
- **THEN** the image is exported to the local Docker daemon

#### Scenario: Export as OCI tarball
- **WHEN** `--output type=oci,dest=image.tar` is specified
- **THEN** the image is saved as an OCI-format tarball at the specified path

#### Scenario: Export as Docker tarball
- **WHEN** `--output type=tar,dest=image.tar` is specified
- **THEN** the image is saved as a Docker-format tarball at the specified path

---

### Requirement: BuildKit Endpoint Configuration
The system SHALL support configuring the BuildKit daemon endpoint.

#### Scenario: Default endpoint
- **WHEN** no `--buildkit` flag is provided
- **THEN** the system uses `unix:///run/buildkit/buildkitd.sock`

#### Scenario: Custom endpoint via flag
- **WHEN** `--buildkit <endpoint>` is provided
- **THEN** the system connects to the specified endpoint

#### Scenario: Endpoint via environment variable
- **WHEN** `BUILDKIT_HOST` environment variable is set
- **THEN** the system uses the environment variable value
- **AND** `--buildkit` flag overrides the environment variable

---

### Requirement: Mandatory Attestations
The system SHALL always generate SBOM and provenance attestations for all builds.

#### Scenario: Attestations always enabled
- **WHEN** `peelbox build` is executed
- **THEN** SBOM and provenance attestations are generated and attached
- **AND** attestations cannot be disabled (security by default)

---

### Requirement: Build Progress Output
The system SHALL display build progress during image building.

#### Scenario: Progress streaming
- **WHEN** a build is in progress
- **THEN** build steps and their status are displayed in real-time
- **AND** cache hit/miss status is shown for each layer

#### Scenario: Quiet mode
- **WHEN** `--quiet` flag is specified
- **THEN** only errors and final image digest are displayed

#### Scenario: Verbose mode
- **WHEN** `--verbose` flag is specified
- **THEN** detailed build logs including command output are displayed

---

### Requirement: Remove Dockerfile Output
The system SHALL remove the Dockerfile generation capability in favor of direct BuildKit building.

#### Scenario: Detect command output
- **WHEN** `peelbox detect` is executed
- **THEN** the output is UniversalBuild specification (JSON/YAML/text)
- **AND** no Dockerfile output option is available

#### Scenario: Build replaces Dockerfile generation
- **WHEN** user needs a container image
- **THEN** they use `peelbox build` to build directly via BuildKit
- **AND** there is no intermediate Dockerfile step

