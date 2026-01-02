# Tasks: Direct BuildKit Daemon Interaction

## Prerequisites
- [ ] **Research BuildKit FileSync protocol** - Understand fsutil packets, bidirectional streaming, session protocol
- [ ] **Study buildkit-proto definitions** - Review Control, FileSync, and Session service protobuf definitions

## Phase 1: BuildKit gRPC Client Foundation (Deliverable: Connection Management)
- [ ] **Add gRPC dependencies to Cargo.toml**:
  - [ ] `tonic = "0.12"` - gRPC framework
  - [ ] `prost = "0.13"` - Protobuf (already in tree, verify version)
  - [ ] `tokio-stream = "0.1"` - Async stream utilities
  - [ ] Verify `buildkit-proto` available from `buildkit-llb` dependency tree
- [ ] **Implement connection module** (`src/buildkit/connection.rs`)
  - [ ] Unix socket connection (default: `/run/buildkit/buildkitd.sock`)
  - [ ] TCP connection with optional TLS support using `tonic::transport::Endpoint`
  - [ ] Connection auto-detection: try standalone BuildKit socket, fall back to Docker daemon
  - [ ] Health check using BuildKit Control service Health RPC
  - [ ] BuildKit version check via Control.Info (require v0.11.0+)
  - [ ] Cross-platform socket paths (Linux, macOS, Windows)
- [ ] **Write unit tests for connection logic** - Test socket path resolution, auto-detection, version validation
- [ ] **Integration test: Connect to BuildKit daemon** - Verify connection with containerized BuildKit

## Phase 2: FileSync Protocol Implementation (Deliverable: File Transfer)
- [ ] **Research fsutil packet format** - Study BuildKit's fsutil.types.Packet structure
- [ ] **Implement FileSync client** (`src/buildkit/filesync.rs`)
  - [ ] Bidirectional gRPC streaming using `tonic::Streaming`
  - [ ] Walk local filesystem with gitignore filtering
  - [ ] Send file stats as fsutil packets (names, sizes, modes)
  - [ ] Respond to daemon's file content requests
  - [ ] Handle diff-based transfer (only send changed files)
  - [ ] Stream file content efficiently (chunking large files)
- [ ] **Implement session module** (`src/buildkit/session.rs`)
  - [ ] Session initialization via Control.Session RPC
  - [ ] Attach FileSync service to session
  - [ ] Session lifecycle management (create, attach services, close)
  - [ ] Error handling and graceful shutdown
- [ ] **Write unit tests for FileSync** - Mock filesystem, test packet generation
- [ ] **Integration test: Transfer build context** - Verify files transferred correctly to BuildKit

## Phase 3: LLB Submission and Build Execution (Deliverable: End-to-End Build)
- [ ] **Implement build execution** (`src/buildkit/session.rs` continued)
  - [ ] Submit LLB definition via Control.Solve RPC
  - [ ] Stream progress updates using StatusResponse
  - [ ] Handle build completion (success/failure)
  - [ ] Retrieve SBOM and provenance attestations
- [ ] **Refactor LLB generation** (`src/buildkit/llb.rs`)
  - [ ] Keep `build()` method for LLB generation
  - [ ] Remove `write_definition()` (frontend stdout protocol)
  - [ ] Add `to_bytes()` method returning LLB protobuf bytes for gRPC
- [ ] **Write unit tests for build execution** - Mock Control.Solve responses
- [ ] **Integration test: Build simple image** - Full workflow with Rust Cargo fixture

## Phase 4: Build Command Implementation (Deliverable: `peelbox build`)
- [ ] **Define build command CLI** (`src/cli/commands.rs`)
  - [ ] `BuildArgs` struct with flags
  - [ ] `--spec` (required) - Path to UniversalBuild JSON (e.g., `universalbuild.json`)
  - [ ] `--tag` (required) - Image tag (e.g., `myapp:latest`)
  - [ ] `--output` - Output type (`docker` or `oci,dest=file.tar`, defaults to `docker`)
  - [ ] `--buildkit` - Override BuildKit daemon address (optional)
  - [ ] `--entrypoint` - Override entrypoint at build time (optional)
  - [ ] `--platform` - Target platform (e.g., `linux/amd64,linux/arm64`, optional)
  - [ ] `--service` - Service name for monorepos (optional)
- [ ] **Implement build command handler** (`src/cli/build.rs`)
  - [ ] Load UniversalBuild spec from `--spec` file (error if missing)
  - [ ] Select service if `--service` provided and spec has multiple services
  - [ ] Connect to BuildKit daemon (auto-detect or use `--buildkit`)
  - [ ] Generate LLB from spec using existing `LLBBuilder`
  - [ ] Create session and transfer build context via FileSync
  - [ ] Submit LLB and stream progress to stdout
  - [ ] Handle build completion and retrieve attestations
  - [ ] Execute output action (Docker or OCI tar)
- [ ] **Remove frontend command** (`src/cli/commands.rs`, `src/main.rs`)
  - [ ] Delete `FrontendArgs` struct
  - [ ] Delete `handle_frontend()` function
  - [ ] Remove `Commands::Frontend` variant
  - [ ] Add clear error if user attempts `peelbox frontend` (suggest `peelbox build`)
- [ ] **Write unit tests for build command** - Test CLI parsing, spec loading, validation
- [ ] **Integration test: Build with both outputs** - Test Docker export and OCI tar export

## Phase 5: Docker Daemon Fallback (Deliverable: Docker Integration)
- [ ] **Implement Docker daemon detection** (`src/buildkit/docker.rs`)
  - [ ] Connect to Docker socket (`/var/run/docker.sock` or platform-specific)
  - [ ] Call Docker API `/info` endpoint to check BuildKit availability
  - [ ] Verify Docker API version >= 1.41 (Docker 23.0+)
  - [ ] Extract BuildKit endpoint from Docker info response
  - [ ] Use Docker's BuildKit if standalone socket not found
- [ ] **Update connection auto-detection** (`src/buildkit/connection.rs`)
  - [ ] Try Unix socket first (`/run/buildkit/buildkitd.sock`)
  - [ ] Fall back to Docker daemon if socket not found
  - [ ] Log which connection type was used (for debugging)
- [ ] **Write unit tests for Docker detection** - Mock Docker API responses, test version checks
- [ ] **Integration test: Docker fallback** - Build without standalone BuildKit, verify Docker used

## Phase 6: Progress and Logging (Deliverable: Real-time Feedback)
- [ ] **Implement progress streaming** (`src/buildkit/progress.rs`)
  - [ ] Parse BuildKit StatusResponse messages from Control.Status stream
  - [ ] Track vertex status (layer operations): started, cached, completed, errored
  - [ ] Render progress bars using `indicatif` crate (or simple text for non-TTY)
  - [ ] Stream build logs from vertex log messages
  - [ ] Calculate and display cache hit ratio
  - [ ] Display final build summary (duration, image size, layers cached)
- [ ] **Add quiet mode** (`--quiet` flag) - Suppress progress, only show final summary and errors
- [ ] **Add verbose mode** (`--verbose` flag) - Show full BuildKit vertex details and internal operations
- [ ] **Write unit tests for progress parsing** - Mock StatusResponse messages, test vertex tracking
- [ ] **Integration test: Progress output** - Build image, capture stdout, verify progress displayed

## Phase 7: Output Formats (Deliverable: Docker and OCI Tar Export)
- [ ] **Implement Docker output** (`src/buildkit/output/docker.rs`)
  - [ ] Use BuildKit exporter with `type=docker` output
  - [ ] Stream image tarball from BuildKit
  - [ ] Load tarball into Docker daemon via `/var/run/docker.sock`
  - [ ] Verify image exists using Docker API inspect
- [ ] **Implement OCI tar export** (`src/buildkit/output/oci.rs`)
  - [ ] Use BuildKit exporter with `type=oci,dest=<path>` output
  - [ ] Stream OCI layout tarball from BuildKit
  - [ ] Write tarball to specified file path
  - [ ] Verify tarball includes SBOM and provenance attestations
- [ ] **Write unit tests for output handlers** - Mock BuildKit exporter responses
- [ ] **Integration test: Both output types** - Build and export to Docker and OCI tar, verify both

## Phase 8: Documentation Updates (Deliverable: Updated Docs)
- [ ] **Update README.md**
  - [ ] Replace frontend command examples with build command
  - [ ] Add 2-step workflow quick start (detect → build)
  - [ ] Document all build command flags
  - [ ] Explain Docker daemon fallback behavior
  - [ ] Add troubleshooting section (BuildKit connection errors)
- [ ] **Update CLAUDE.md**
  - [ ] Remove frontend protocol section
  - [ ] Add gRPC client architecture section with FileSync protocol details
  - [ ] Update all buildctl references to `peelbox build`
  - [ ] Document connection types and auto-detection order
  - [ ] Add file transfer protocol complexity notes

## Phase 9: Testing and Validation (Deliverable: Full Test Coverage)
- [ ] **E2E tests for connection types**
  - [ ] Unix socket (standalone BuildKit container)
  - [ ] Docker daemon fallback (stop standalone, use Docker)
- [ ] **E2E tests for output types**
  - [ ] Docker export (default behavior)
  - [ ] OCI tarball export
- [ ] **Test monorepo service selection** - Verify `--service` flag with multi-service spec
- [ ] **Test entrypoint override** - Build with `--entrypoint`, verify container uses override
- [ ] **Test platform targeting** - Build for `linux/amd64` and verify platform in image config
- [ ] **Performance comparison** - Compare build times: `peelbox build` vs `buildctl` pipeline
- [ ] **Run all existing tests** - Ensure no regressions in detection, LLB generation, fixtures
- [ ] **Test error handling** - Verify clear errors for missing BuildKit, invalid spec, connection failures

## Phase 10: Polish and CI Updates (Deliverable: Production Ready)
- [ ] **Error message improvements**
  - [ ] Clear message if BuildKit not found (show connection attempts, install instructions)
  - [ ] Helpful error if `peelbox frontend` attempted (suggest `peelbox build --spec`)
  - [ ] Connection troubleshooting (socket permissions, daemon not running, version mismatch)
  - [ ] File transfer errors (context too large, permission denied)
- [ ] **CI/CD updates**
  - [ ] Update GitHub Actions workflows to use 2-step workflow (detect → build)
  - [ ] Update container integration tests to use `peelbox build`
  - [ ] Verify CI builds work with both standalone and Docker daemon BuildKit
- [ ] **Final validation**
  - [ ] All tests passing (unit, integration, E2E)
  - [ ] Documentation complete (README, CLAUDE.md)
  - [ ] Version bumped to 0.4.0 in Cargo.toml (breaking change)

## Dependencies and Parallelization

**Can run in parallel:**
- Phase 1 (Connection) + Phase 5 (Docker fallback) - Independent connection logic
- Phase 6 (Progress) - Can be developed alongside Phase 7 (Output)
- Phase 7 (Output formats) - Docker and OCI outputs are independent

**Must be sequential:**
- Phase 1 → Phase 2 → Phase 3 → Phase 4 (Connection → FileSync → LLB Submission → Build command)
- Phase 4 → Phase 8 (Build command must exist before updating docs)
- Phase 9 depends on all phases (testing validates everything)
- Phase 10 depends on Phase 9 (polish after testing complete)

**Critical path:** Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 9 → Phase 10 (Connection → FileSync → Build Execution → Build Command → Testing → Polish)
