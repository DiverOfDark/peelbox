# wolfi-images Specification

## Purpose
TBD - created by archiving change add-buildkit-wolfi-frontend. Update Purpose after archive.
## Requirements
### Requirement: Wolfi Base Image
The system SHALL use `cgr.dev/chainguard/wolfi-base` as the single base image for all container builds.

#### Scenario: Build stage base image
- **WHEN** generating a build stage
- **THEN** the base image is `cgr.dev/chainguard/wolfi-base`
- **AND** language toolchains are installed via apk

#### Scenario: Runtime stage base image
- **WHEN** generating a runtime stage
- **THEN** the base image is `cgr.dev/chainguard/wolfi-base`
- **AND** only required runtime packages are installed via apk

---

### Requirement: APK Package Installation
The system SHALL install all packages using the apk package manager with Wolfi package names.

#### Scenario: Install build toolchain
- **WHEN** build stage requires a language toolchain
- **THEN** the system generates `apk add --no-cache <toolchain-package>`
- **AND** uses Wolfi package names (e.g., `rust`, `nodejs-22`, `python-3.12`, `go`, `openjdk-21`)

#### Scenario: Install runtime dependencies
- **WHEN** runtime stage requires dependencies
- **THEN** the system generates `apk add --no-cache <packages>`
- **AND** uses minimal package set for runtime (e.g., `glibc`, `ca-certificates`)

#### Scenario: Install build dependencies
- **WHEN** build requires compilation tools
- **THEN** the system installs `build-base` for C/C++ compilation support

---

### Requirement: Wolfi Package Name Guidance
The system SHALL guide the LLM to use correct Wolfi package names for common dependencies.

#### Scenario: Rust project packages
- **WHEN** language is "rust"
- **THEN** build stage installs `rust` package
- **AND** runtime stage installs `glibc` and `ca-certificates` if needed

#### Scenario: Node.js project packages
- **WHEN** language is "nodejs"
- **THEN** build and runtime stages install `nodejs-22` or appropriate version

#### Scenario: Python project packages
- **WHEN** language is "python"
- **THEN** build and runtime stages install `python-3.12` or appropriate version
- **AND** includes `py3-pip` if pip is needed

#### Scenario: Go project packages
- **WHEN** language is "go"
- **THEN** build stage installs `go` package
- **AND** runtime stage is minimal (static binary) or includes `glibc` for CGO

#### Scenario: Java project packages
- **WHEN** language is "java"
- **THEN** build stage installs `openjdk-21` (full JDK)
- **AND** runtime stage installs `openjdk-21-jre` (JRE only)

---

### Requirement: Minimal Runtime Image
The system SHALL generate runtime stages with minimal package footprint.

#### Scenario: Static binary runtime
- **WHEN** the built artifact is a statically linked binary
- **THEN** runtime stage only includes `ca-certificates` if TLS is needed
- **AND** no additional packages are installed

#### Scenario: Dynamic binary runtime
- **WHEN** the built artifact requires dynamic linking
- **THEN** runtime stage includes `glibc` and required shared libraries
- **AND** only essential runtime packages are included

