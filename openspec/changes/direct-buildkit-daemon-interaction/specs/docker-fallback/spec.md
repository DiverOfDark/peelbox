# docker-fallback Specification Delta

## Purpose
Define Docker daemon fallback behavior when standalone BuildKit is unavailable, enabling seamless use of Docker's built-in BuildKit.

## ADDED Requirements

### Requirement: Docker Daemon Detection
The system SHALL detect and use Docker daemon's built-in BuildKit when standalone BuildKit is unavailable.

#### Scenario: Detect Docker daemon with BuildKit
- **WHEN** standalone BuildKit socket not found at `/run/buildkit/buildkitd.sock`
- **AND** Docker socket exists at `/var/run/docker.sock`
- **THEN** the system connects to Docker daemon
- **AND** checks Docker API version for BuildKit support
- **AND** extracts BuildKit endpoint from Docker daemon info

#### Scenario: Docker daemon BuildKit enabled
- **WHEN** Docker daemon connection succeeds
- **AND** Docker API version is 1.41+ (Docker 23.0+)
- **THEN** the system confirms BuildKit is available
- **AND** proceeds to use Docker daemon's BuildKit for builds

#### Scenario: Docker daemon without BuildKit
- **WHEN** Docker daemon connection succeeds
- **AND** Docker API version is < 1.41 (older than Docker 23.0)
- **THEN** the system returns an error explaining BuildKit not supported
- **AND** suggests upgrading Docker to version 23.0+ or Docker Desktop 4.17+

#### Scenario: Docker daemon not running
- **WHEN** Docker socket `/var/run/docker.sock` does not exist
- **OR** Docker daemon is not responding
- **THEN** the system skips Docker fallback
- **AND** returns error indicating no BuildKit available (standalone or Docker)

---

### Requirement: Docker Socket Permissions
The system SHALL handle Docker socket permission errors gracefully.

#### Scenario: Docker socket permission denied
- **WHEN** Docker socket exists but current user lacks read/write permissions
- **THEN** the system returns an error explaining permission issue
- **AND** suggests adding user to `docker` group or using `sudo`
- **AND** provides platform-specific instructions (Linux vs macOS)

---

### Requirement: BuildKit Endpoint Extraction
The system SHALL extract BuildKit endpoint from Docker daemon for gRPC connection.

#### Scenario: Extract BuildKit endpoint from Docker info
- **WHEN** Docker daemon has BuildKit enabled
- **THEN** the system queries Docker info API
- **AND** extracts BuildKit builder endpoint (e.g., `tcp://127.0.0.1:buildkit-port`)
- **AND** uses extracted endpoint for gRPC connection

#### Scenario: BuildKit endpoint not found in Docker info
- **WHEN** Docker info does not contain BuildKit endpoint
- **THEN** the system returns an error
- **AND** suggests enabling BuildKit in Docker daemon config

---

### Requirement: Fallback Indication
The system SHALL clearly indicate when Docker daemon fallback is used.

#### Scenario: Log Docker fallback usage
- **WHEN** standalone BuildKit not found and Docker daemon used
- **THEN** the system logs an info message:
  ```
  Using Docker daemon's BuildKit (standalone BuildKit not found)
  ```
- **AND** build proceeds normally with Docker daemon BuildKit

#### Scenario: No indication when standalone used
- **WHEN** standalone BuildKit is available and used
- **THEN** no fallback message is logged (default behavior)

---

### Requirement: Docker Daemon API Version Check
The system SHALL validate Docker daemon API version before using BuildKit.

#### Scenario: Compatible Docker version
- **WHEN** Docker daemon API version is 1.41 or higher
- **THEN** BuildKit is considered available

#### Scenario: Incompatible Docker version
- **WHEN** Docker daemon API version is lower than 1.41
- **THEN** the system returns an error:
  ```
  Error: Docker daemon found but BuildKit not supported

  Your Docker version: 20.10 (API 1.40)
  Required version: 23.0+ (API 1.41+)

  Upgrade Docker:
    Docker Desktop: Update to 4.17+
    Docker Engine: Update to 23.0+

  Or install standalone BuildKit:
    macOS: brew install buildkit
    Linux: sudo apt install buildkit
  ```

---

### Requirement: Docker Context Support
The system SHALL respect active Docker context when using Docker daemon fallback.

#### Scenario: Use active Docker context
- **WHEN** user has active Docker context (e.g., remote Docker host)
- **AND** Docker daemon fallback is used
- **THEN** the system connects to Docker daemon from active context
- **AND** respects TLS settings from context

#### Scenario: No active context
- **WHEN** no Docker context is active
- **THEN** the system uses default Docker socket (`/var/run/docker.sock`)

---

### Requirement: Fallback Order
The system SHALL follow a deterministic fallback order for BuildKit connections.

#### Scenario: Connection attempt order
- **WHEN** no `--buildkit` flag is specified
- **THEN** the system attempts connections in this order:
  1. Unix socket: `/run/buildkit/buildkitd.sock`
  2. Docker daemon: `/var/run/docker.sock` (if API 1.41+)
- **AND** the first successful connection is used
- **AND** subsequent attempts are skipped

#### Scenario: Explicit address skips fallback
- **WHEN** `--buildkit` flag is specified
- **THEN** only the specified address is attempted
- **AND** no fallback occurs if connection fails

---

### Requirement: Cross-Platform Socket Paths
The system SHALL use correct socket paths for each operating system.

#### Scenario: Linux socket paths
- **WHEN** running on Linux
- **THEN** standalone BuildKit socket: `/run/buildkit/buildkitd.sock`
- **AND** Docker socket: `/var/run/docker.sock`

#### Scenario: macOS socket paths
- **WHEN** running on macOS
- **THEN** standalone BuildKit socket: `/run/buildkit/buildkitd.sock` (if installed)
- **AND** Docker socket: `/var/run/docker.sock` (Docker Desktop)

#### Scenario: Windows socket paths
- **WHEN** running on Windows
- **THEN** Docker named pipe: `//./pipe/docker_engine`
- **AND** standalone BuildKit: `tcp://127.0.0.1:1234` (no Unix sockets on Windows)

## MODIFIED Requirements

None - this is a new capability.

## REMOVED Requirements

None - this is a new capability.
