# buildkit-frontend Specification Changes

## MODIFIED Requirements

### Requirement: LLB Graph Generation
The system SHALL generate BuildKit LLB graphs from UniversalBuild specifications using wolfi-base with optimized layer separation and no code duplication.

#### Scenario: Generate LLB for any language
- **WHEN** a UniversalBuild specification is provided
- **THEN** the system generates an LLB graph with build stage using `cgr.dev/chainguard/wolfi-base`
- **AND** installs language toolchain via `apk add`
- **AND** runtime stage uses `cgr.dev/chainguard/wolfi-base` with minimal runtime packages
- **AND** all base image loads reuse the same variable (no duplicate loads)

#### Scenario: Generate LLB with cache mounts
- **WHEN** a UniversalBuild specifies cache paths in build.cache
- **THEN** the LLB graph includes cache mount operations for each path
- **AND** cache mounts use `sharing=shared` mode for concurrent builds

#### Scenario: Generate LLB with multi-stage build
- **WHEN** a UniversalBuild specifies separate build and runtime stages
- **THEN** the LLB graph creates two distinct stage definitions
- **AND** artifacts are copied from build stage to runtime stage

#### Scenario: Separate build command layers
- **WHEN** multiple build commands are specified in `build.commands`
- **THEN** each command is executed as a separate BuildKit layer using direct `.run()` calls
- **AND** BuildKit can cache each command layer independently
- **AND** subsequent builds skip unchanged command layers

#### Scenario: No code duplication in LLB generation
- **WHEN** generating LLB graph
- **THEN** base image loads are reused (no duplicate `WOLFI_BASE_IMAGE` or `BusyBox` loads)
- **AND** path normalization uses a single `normalize_path()` helper function
- **AND** directory detection uses a single `is_directory()` helper function

---

## ADDED Requirements

### Requirement: Artifacts Extracted from Copy Specification
The system SHALL extract build artifacts from the runtime.copy specification instead of a separate artifacts field.

#### Scenario: Extract artifacts from runtime.copy
- **WHEN** generating LLB for artifact copying
- **THEN** the system reads artifact paths from `runtime.copy[].from` fields
- **AND** does not use a separate `build.artifacts` field
- **AND** all artifacts are specified once in runtime.copy

---

### Requirement: Service Selection for Monorepos
The system SHALL require explicit service selection for multi-service specifications.

#### Scenario: Single service specification
- **WHEN** UniversalBuild array contains exactly one service
- **THEN** the system builds that service without requiring `--service` flag

#### Scenario: Multi-service requires flag
- **WHEN** UniversalBuild array contains multiple services
- **AND** `--service` flag is not provided
- **THEN** the system returns an error listing available services
- **AND** the error message includes: "Multiple services detected. Use --service flag to specify which to build."

#### Scenario: Service selection with flag
- **WHEN** UniversalBuild array contains multiple services
- **AND** `--service servicename` flag is provided
- **THEN** the system builds only the specified service
- **AND** returns an error if the service name is not found

#### Scenario: No fallback to first service
- **WHEN** multiple services are detected
- **AND** `--service` flag is missing
- **THEN** the system MUST NOT default to building the first service
- **AND** MUST return an error requiring explicit selection
