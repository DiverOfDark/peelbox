# buildkit-grpc Specification Delta

## Purpose
Define BuildKit gRPC client capabilities for direct daemon communication, replacing the frontend protocol with native gRPC interactions for real-time build control.

## ADDED Requirements

### Requirement: BuildKit Daemon Connection
The system SHALL connect to BuildKit daemon via gRPC with automatic fallback between connection types.

#### Scenario: Auto-detect connection
- **WHEN** no `--buildkit` flag is specified
- **THEN** the system attempts connection in this order:
  1. Unix socket at `/run/buildkit/buildkitd.sock`
  2. Docker daemon's BuildKit (if Docker API available and BuildKit enabled)
- **AND** the first successful connection is used for the build session
- **AND** connection is validated with health check before proceeding

#### Scenario: Explicit Unix socket
- **WHEN** `--buildkit unix:///run/buildkit/buildkitd.sock` is specified
- **THEN** the system connects only to that Unix socket
- **AND** returns clear error if socket doesn't exist or daemon isn't running

#### Scenario: TCP with TLS
- **WHEN** `--buildkit tcp://remote.example.com:1234` is specified
- **THEN** the system connects via TCP to the specified host and port
- **AND** automatically detects and uses TLS if the endpoint requires it
- **AND** validates server certificate for secure connections

#### Scenario: Docker container BuildKit
- **WHEN** `--buildkit docker-container://buildkitd` is specified
- **THEN** the system connects through Docker's BuildKit container integration
- **AND** uses Docker socket to proxy gRPC requests

#### Scenario: Docker daemon BuildKit fallback
- **WHEN** standalone BuildKit socket not found
- **AND** Docker daemon is running at `/var/run/docker.sock`
- **AND** Docker daemon has BuildKit support enabled
- **THEN** the system connects to Docker daemon's embedded BuildKit
- **AND** logs a message indicating Docker fallback was used

#### Scenario: Connection failure with helpful error
- **WHEN** all connection attempts fail
- **THEN** the system returns an error listing all attempted connection methods
- **AND** provides installation instructions for BuildKit
- **AND** suggests Docker upgrade if Docker found but BuildKit not supported

---

### Requirement: BuildKit Version Validation
The system SHALL validate BuildKit daemon version meets minimum requirements before starting builds.

#### Scenario: Valid BuildKit version
- **WHEN** connecting to BuildKit daemon version 0.11.0 or later
- **THEN** the connection proceeds and build session can be established

#### Scenario: Outdated BuildKit version
- **WHEN** connecting to BuildKit daemon version older than 0.11.0
- **THEN** the system returns an error explaining version requirement
- **AND** states that v0.11.0+ is required for SBOM/provenance support
- **AND** provides upgrade instructions for Docker Desktop, Docker Engine, or standalone BuildKit

---

### Requirement: Connection Pooling
The system SHALL reuse BuildKit connections across multiple builds to reduce overhead.

#### Scenario: Reuse connection for multiple builds
- **WHEN** multiple build commands are executed sequentially
- **THEN** the same gRPC connection is reused if still healthy
- **AND** connection health is validated before each reuse

#### Scenario: Reconnect on stale connection
- **WHEN** a pooled connection becomes stale or unhealthy
- **THEN** the system automatically establishes a new connection
- **AND** retries the build operation transparently

---

### Requirement: Build Session Management
The system SHALL manage BuildKit sessions for LLB submission, context transfer, and output handling.

#### Scenario: Create build session
- **WHEN** a build is initiated
- **THEN** a new BuildKit session is created via gRPC
- **AND** session is assigned a unique ID for tracking

#### Scenario: Transfer build context
- **WHEN** a session is established
- **THEN** the system sends build context files to BuildKit
- **AND** respects `.gitignore` patterns to minimize transfer size
- **AND** uses HTTP/2 tunneling for efficient file synchronization

#### Scenario: Submit LLB definition
- **WHEN** build context is transferred
- **THEN** the generated LLB definition is submitted to the session
- **AND** BuildKit validates and begins executing the build graph

#### Scenario: Session cleanup on completion
- **WHEN** build completes (success or failure)
- **THEN** the session is properly closed and resources released

#### Scenario: Session cleanup on cancellation
- **WHEN** user cancels build (Ctrl-C)
- **THEN** the session is cancelled gracefully via gRPC
- **AND** BuildKit stops ongoing operations

---

### Requirement: Real-time Progress Streaming
The system SHALL stream build progress updates from BuildKit in real-time.

#### Scenario: Stream layer build progress
- **WHEN** a build is running
- **THEN** the system receives status updates for each layer operation
- **AND** displays progress bars showing current operation and percentage
- **AND** indicates whether layers are cached or rebuilt

#### Scenario: Stream build logs
- **WHEN** build commands execute
- **THEN** stdout and stderr from build processes are streamed to user
- **AND** logs are displayed in real-time with layer context

#### Scenario: Show cache hit ratio
- **WHEN** build completes
- **THEN** the system summarizes cache hit ratio (cached layers vs rebuilt)
- **AND** displays total build time and final image size

#### Scenario: Quiet mode suppresses progress
- **WHEN** `--quiet` flag is used
- **THEN** progress bars and streaming logs are suppressed
- **AND** only final summary and errors are shown

---

### Requirement: gRPC Error Handling
The system SHALL handle gRPC errors gracefully with actionable error messages.

#### Scenario: Connection timeout
- **WHEN** gRPC connection attempt exceeds timeout
- **THEN** system returns error explaining timeout
- **AND** suggests checking if daemon is running and network is accessible

#### Scenario: Build failure
- **WHEN** BuildKit returns build error status
- **THEN** system displays the failing layer and command
- **AND** streams full error logs from BuildKit
- **AND** returns non-zero exit code

#### Scenario: Daemon crash during build
- **WHEN** BuildKit daemon crashes mid-build
- **THEN** system detects gRPC connection loss
- **AND** returns error explaining daemon unavailability
- **AND** suggests restarting daemon

## MODIFIED Requirements

### Requirement: LLB Graph Generation
The system SHALL generate BuildKit LLB graphs from UniversalBuild specifications for submission via gRPC (previously stdout).

#### Scenario: Generate LLB bytes for gRPC
- **WHEN** a UniversalBuild specification is provided
- **THEN** the system generates an LLB graph as protobuf bytes
- **AND** LLB is returned as `Vec<u8>` for gRPC submission
- **AND** gitignore patterns are embedded in LLB (no filesystem dependency)

## REMOVED Requirements

### ~~Requirement: BuildKit Frontend Protocol~~
**REASON:** Replaced by direct gRPC client - frontend stdout protocol no longer used.

### ~~Requirement: LLB Output to Stdout~~
**REASON:** LLB is now submitted directly via gRPC session, not written to stdout for buildctl piping.
