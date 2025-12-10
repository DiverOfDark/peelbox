## ADDED Requirements

### Requirement: Build Command
The system SHALL provide a `build` command that builds container images via BuildKit.

#### Scenario: Build command with tag
- **WHEN** `aipack build --tag image:tag` is executed
- **THEN** the system detects the repository, generates LLB, and builds the image
- **AND** the image is tagged with the specified name

#### Scenario: Build and push
- **WHEN** `aipack build --tag registry/image:tag --push` is executed
- **THEN** the system builds and pushes the image to the registry
- **AND** authentication uses Docker config or environment credentials

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

### Requirement: Attestation Flags
The system SHALL support flags to control SBOM and provenance attestation generation.

#### Scenario: Attestations enabled by default
- **WHEN** `aipack build` is executed without attestation flags
- **THEN** SBOM and provenance attestations are generated and attached

#### Scenario: Disable SBOM
- **WHEN** `--no-sbom` flag is specified
- **THEN** no SBOM attestation is generated

#### Scenario: Disable provenance
- **WHEN** `--no-provenance` flag is specified
- **THEN** no provenance attestation is generated

#### Scenario: Disable all attestations
- **WHEN** `--no-attestations` flag is specified
- **THEN** neither SBOM nor provenance attestations are generated

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
- **WHEN** `aipack detect` is executed
- **THEN** the output is UniversalBuild specification (JSON/YAML/text)
- **AND** no Dockerfile output option is available

#### Scenario: Build replaces Dockerfile generation
- **WHEN** user needs a container image
- **THEN** they use `aipack build` to build directly via BuildKit
- **AND** there is no intermediate Dockerfile step
