# Design: Direct BuildKit Daemon Interaction

## Architecture Overview

### Current Architecture (Frontend Protocol)
```
┌─────────────┐
│   peelbox   │
│   detect    │
│             │
│  (Pipeline) │
└──────┬──────┘
       │ UniversalBuild JSON
       ▼
┌─────────────┐
│   peelbox   │
│  frontend   │
│             │
│ (LLB Gen)   │
└──────┬──────┘
       │ LLB protobuf (stdout)
       ▼
┌─────────────┐    gRPC     ┌─────────────┐
│   buildctl  │◄───────────►│  buildkitd  │
│    (CLI)    │             │   (daemon)  │
└──────┬──────┘             └──────┬──────┘
       │ Docker tar                │
       ▼                           ▼
┌─────────────┐             ┌─────────────┐
│   docker    │             │   Image     │
│    load     │             │   Registry  │
└─────────────┘             └─────────────┘

Limitations:
- 3 separate commands (detect, frontend, buildctl)
- No real-time progress
- Requires buildctl installation
- Cannot customize outputs without complex buildctl flags
```

### New Architecture (Direct gRPC - 2-Step Workflow)
```
Step 1: Detection
┌─────────────────────────────────────────────┐
│           peelbox detect                    │
│                                             │
│  ┌──────────────────────────────────────┐  │
│  │     Pipeline (9 phases)              │  │
│  │  - Scan → Classify → Structure       │  │
│  │  - Dependencies → Build Order        │  │
│  │  - Service Analysis → Assemble       │  │
│  └──────────────┬───────────────────────┘  │
└─────────────────┼──────────────────────────┘
                  │ universalbuild.json
                  ▼ (saved to file)

Step 2: Building
┌─────────────────────────────────────────────┐
│           peelbox build                     │
│           --spec universalbuild.json        │
│                                             │
│  ┌─────────────┐      ┌─────────────┐     │
│  │  Load Spec  │──────►│   LLB Gen   │     │
│  │  (JSON)     │ spec │ (buildkit/) │     │
│  └─────────────┘      └──────┬──────┘     │
│                              │ LLB bytes   │
│                              ▼             │
│  ┌──────────────────────────────────────┐ │
│  │      BuildKit gRPC Client            │ │
│  │  (buildkit/client.rs)                │ │
│  │                                      │ │
│  │  • Connection (Unix/Docker)          │ │
│  │  • Session management                │ │
│  │  • FileSync protocol (context)       │ │
│  │  • Progress streaming                │ │
│  │  • Output handling (Docker, OCI)     │ │
│  └─────────────┬────────────────────────┘ │
└────────────────┼──────────────────────────┘
                 │ gRPC:
                 │ 1. Session init
                 │ 2. FileSync (build context)
                 │ 3. LLB submission
                 │ 4. Progress stream
                 ▼
     ┌───────────────────────┐
     │  BuildKit Daemon      │
     │  (buildkitd or        │
     │   Docker daemon)      │
     │                       │
     │  Auto-detect:         │
     │  1. Unix socket       │
     │  2. Docker daemon     │
     │  3. TCP (if --addr)   │
     └──────────┬────────────┘
                │ Export
       ┌────────┴────────┐
       │                 │
       ▼                 ▼
┌─────────────┐   ┌─────────────┐
│   Docker    │   │  OCI Tar    │
│   Daemon    │   │  Archive    │
└─────────────┘   └─────────────┘

Benefits:
- 2-step workflow (detect → build, clear separation)
- Real-time progress and logs
- Direct output control (Docker, OCI tar)
- No external dependencies (buildctl not needed)
- UniversalBuild spec can be edited/version controlled between steps
```

## Key Design Decisions

### 1. BuildKit Client Implementation Approach

**Decision:** Implement using `tonic` + `buildkit-proto` (not `buildkit-client` crate)

**Rationale:**
- Full control over gRPC client behavior and error handling
- buildkit-proto already in dependency tree (via buildkit-llb)
- Smaller binary size (only include what we need)
- Learning opportunity for BuildKit internals
- Flexibility to optimize for peelbox-specific use cases

**Alternatives Considered:**
- **Use `buildkit-client = "0.1.4"`**: Faster to implement but less control, larger dependency
- **Shell out to `buildctl`**: Defeats purpose of improvement, still requires buildctl installation

**Trade-offs:**
- More development effort (2-3x longer to implement session protocol)
- Need to understand BuildKit session protocol and FileSync internals
- Must implement fsutil packet handling ourselves

**Mitigations:**
- Well-documented BuildKit proto definitions
- Reference implementation available (buildkit-client source code)

### 2. Connection Auto-Detection Strategy

**Decision:** Tiered fallback with explicit override

**Priority Order:**
1. **Explicit `--buildkit` flag** (if provided) - User knows best
2. **Unix socket** (`/run/buildkit/buildkitd.sock`) - Standalone BuildKit
3. **Docker daemon BuildKit** (via `/var/run/docker.sock`) - Docker 23.0+
4. **Error with installation instructions** - Clear guidance

**Rationale:**
- Maximizes "just works" experience for most users
- Docker daemon increasingly common (built-in to Docker Desktop)
- Standalone BuildKit preferred for performance (dedicated daemon)
- Explicit override for advanced use cases (remote builders, custom sockets)

**Edge Cases:**
- Both standalone and Docker available → prefer standalone (better isolation)
- Docker daemon without BuildKit support → clear error message
- No BuildKit available → suggest installation options

### 3. FileSync Protocol Implementation

**Decision:** Implement BuildKit's FileSync protocol for build context transfer

**Protocol Overview:**
BuildKit uses a sophisticated file synchronization protocol over gRPC for transferring build context from client to daemon. This is NOT a simple file upload - it's an optimization-focused bidirectional streaming protocol.

**FileSync Protocol Flow:**
```
Client                                  BuildKit Daemon
  │                                           │
  │─────── DiffCopy() stream ────────────────►│
  │  1. Send file stats (names, sizes, modes) │
  │                                           │
  │◄──── Request specific files ──────────────│
  │  2. Daemon requests only needed files     │
  │                                           │
  │─────── Stream file content ──────────────►│
  │  3. Send requested file data as packets   │
  │                                           │
  │◄──── Acknowledge completion ──────────────│
```

**Key Components:**
1. **fsutil.types.Packet**: Wire format for file metadata and content
2. **Bidirectional streaming**: Client and daemon exchange messages
3. **Diff-based transfer**: Only changed files since last build
4. **Selective transfer**: Daemon only requests files needed by LLB (ADD/COPY operations)
5. **Gitignore filtering**: Applied client-side before stat generation

**Implementation Challenges:**
- **Packet format**: Must correctly serialize/deserialize fsutil packets (non-trivial protobuf)
- **Streaming coordination**: Coordinating request/response over bidirectional stream
- **File chunking**: Large files must be chunked efficiently
- **Error recovery**: Handling partial transfer failures gracefully

**Sources:**
- [BuildKit filesync package](https://pkg.go.dev/github.com/moby/buildkit/session/filesync) - FileSync gRPC service definition
- [filesync.go implementation](https://github.com/moby/buildkit/blob/master/session/filesync/filesync.go) - Reference implementation
- [BuildKit Issue #1432](https://github.com/moby/buildkit/issues/1432) - Session protocol improvements

**Alternative Considered:**
- **Full context upload every build**: Simpler but 99.9% slower for large repositories

**Why This Is Complex:**
Unlike a simple HTTP file upload, FileSync requires:
- Understanding BuildKit's session lifecycle
- Implementing stateful bidirectional gRPC streaming
- Handling fsutil packet protocol (not standard protobuf)
- Coordinating concurrent file reads with async I/O

This is the primary technical risk of this change.

### 4. Frontend Command Removal vs Deprecation

**Decision:** Complete removal (not deprecation)

**Rationale:**
- Frontend protocol provides no unique value over `peelbox build`
- Maintaining both increases complexity (2 code paths for same goal)
- Clear migration path exists (simple 1:1 replacement)
- Version bump to 0.4.0 signals breaking change

**Migration Support:**
- Detailed MIGRATION.md with before/after examples
- CI/CD migration snippets
- Error message if `frontend` subcommand attempted (with migration link)

**Alternatives Considered:**
- **Deprecate for 1-2 versions**: Delays simplification, confuses users about "right way"
- **Keep forever**: Maintenance burden, splits ecosystem (some use frontend, some use build)

### 5. 2-Step Workflow (Detect → Build)

**Decision:** Require `--spec` flag (no auto-detection in build command)

**Rationale:**
- Clear separation of concerns: detection vs building
- UniversalBuild spec can be edited/reviewed between steps
- Spec can be version-controlled (commit after detection)
- Enables CI/CD caching (cache spec, rebuild without re-detecting)
- Simpler build command implementation (no detection pipeline in build)

**Workflow:**
```bash
# Step 1: Detect and generate spec (run once, or when repo changes)
peelbox detect . > universalbuild.json

# Step 2: Build from spec (run many times)
peelbox build --spec universalbuild.json --tag app:latest
peelbox build --spec universalbuild.json --tag app:test --entrypoint /bin/sh
```

**Alternative Considered:**
- **1-step workflow**: `peelbox build .` auto-detects - simpler UX but mixes concerns

**Trade-off:**
- 2 commands vs 1 command
- BUT: clearer workflow, spec editing capability, better for iteration

### 6. Output Types (Docker and OCI Tar Only)

**Decision:** Support Docker export and OCI tar only - defer registry push to future version

**Rationale:**
- Docker export covers 90% of use cases (local development)
- OCI tar covers CI/CD and airgapped environments
- Registry push adds complexity:
  - Credential management (Docker config.json parsing)
  - Progress tracking for multi-layer push
  - Registry API error handling
  - TLS certificate validation
- Can be added in v0.5.0 after core functionality proven

**Supported Outputs:**
```bash
# Default: Docker daemon (--output type=docker)
peelbox build --spec spec.json --tag app:latest

# OCI tarball
peelbox build --spec spec.json --tag app:latest --output type=oci,dest=app.tar
```

**Deferred to Future:**
- Registry push (`--push` flag)
- Multi-platform manifest creation
- Remote cache export

### 7. Progress Streaming Implementation

**Decision:** Use BuildKit status stream + `indicatif` for rendering

**Architecture:**
```rust
// BuildKit status stream → Progress parser → indicatif rendering
session.build(llb)
    .status_stream()
    .map(|status| parse_progress(status))
    .for_each(|progress| update_ui(progress))
```

**Progress Types:**
- **Layer operations**: Building, pulling, cached
- **Digest computation**: Hashing artifacts
- **Push progress**: Upload percentage per layer
- **Final summary**: Image size, build time

**Quiet Mode:**
- Suppress progress bars
- Only show errors and final summary
- Ideal for CI/CD

**Verbose Mode:**
- Show full BuildKit operation log
- Useful for debugging connection/build issues

### 8. Output Type Handling

**Decision:** BuildKit native outputs (Docker and OCI only initially)

**Supported Outputs:**
```bash
# Docker daemon (default)
peelbox build --spec universalbuild.json --tag app:latest

# OCI tarball
peelbox build --spec universalbuild.json --tag app:latest --output type=oci,dest=app.tar

# Platform targeting
peelbox build --spec universalbuild.json --tag app:latest --platform linux/amd64
```

**Implementation:**
- Use BuildKit's built-in output exporters (no custom code)
- Leverage existing SBOM/provenance generation
- No registry authentication needed (no push support yet)

**Deferred:**
- Registry push support (--push flag)
- Multi-platform manifest creation
- Custom export formats

## Data Flow

### Build Command Execution Flow
```
1. CLI Parsing (clap)
   ├─ --spec (required) - Path to UniversalBuild JSON
   ├─ --tag (required) - Image tag
   ├─ --output (optional) - Output type (defaults to docker)
   ├─ --service (optional) - Service name for monorepos
   └─ Connection options (--buildkit, optional)

2. UniversalBuild Loading
   ├─ Load spec from --spec file path (error if missing/invalid)
   └─ Select service if --service provided and spec has multiple

3. BuildKit Connection
   ├─ Try explicit --buildkit address
   ├─ Try Unix socket (/run/buildkit/buildkitd.sock)
   ├─ Try Docker daemon (/var/run/docker.sock)
   └─ Error if none available

4. LLB Generation
   ├─ LLBBuilder::new(spec)
   └─ Returns LLB protobuf bytes

5. Build Session
   ├─ Create session with BuildKit via Control.Session RPC
   ├─ Attach FileSync service to session
   ├─ Transfer build context via FileSync.DiffCopy (bidirectional stream)
   │  ├─ Walk filesystem with gitignore filtering
   │  ├─ Send file stats as fsutil packets
   │  ├─ Daemon requests needed files
   │  └─ Stream file content in response
   ├─ Submit LLB definition via Control.Solve RPC
   ├─ Stream progress updates from Control.Status → stdout
   └─ Await build completion (success or error)

6. Output Handling
   ├─ If --output type=docker (default) → load to Docker daemon
   ├─ If --output type=oci,dest=file.tar → save OCI tarball
   └─ Show completion summary (duration, size, cache ratio)

7. Cleanup
   └─ Close session, return exit code
```

### Error Handling Strategy

**Connection Errors:**
```
Error: Failed to connect to BuildKit daemon

Tried:
  ✗ Unix socket: /run/buildkit/buildkitd.sock (not found)
  ✗ Docker daemon: /var/run/docker.sock (connected, but no BuildKit support)

Install BuildKit:
  macOS:  brew install buildkit
  Linux:  sudo apt install buildkit
  Docker: Upgrade to Docker Desktop 4.17+ or Docker Engine 23.0+

Or start standalone BuildKit:
  docker run -d --privileged -p 1234:1234 moby/buildkit:latest --addr tcp://0.0.0.0:1234
  peelbox build --buildkit tcp://127.0.0.1:1234 ...
```

**Build Errors:**
- Stream BuildKit error logs directly
- Highlight failed layer/command
- Suggest fixes for common issues (missing packages, build failures)

**Version Errors:**
```
Error: BuildKit version too old

Your BuildKit version: 0.10.6
Required version: 0.11.0+

Reason: SBOM and provenance generation requires BuildKit 0.11.0+

Upgrade:
  Docker Desktop: Update to 4.17+
  Docker Engine: Update to 23.0+
  Standalone: docker pull moby/buildkit:latest
```

## Testing Strategy

### Unit Tests
- Connection logic (mock gRPC calls)
- Session management (mock BuildKit responses)
- Progress parsing (mock status updates)
- Output type selection

### Integration Tests
- Connect to real BuildKit container (via testcontainers)
- Build simple images (Rust, Node.js)
- Test all output types (Docker, push to local registry, OCI tar)
- Test connection fallback (disable standalone, use Docker)

### E2E Tests
- Build and run containers from all test fixtures
- Verify SBOM and provenance attached
- Test monorepo service selection
- Test entrypoint override

### Performance Tests
- Compare build times: `peelbox build` vs `buildctl` pipeline
- Measure progress streaming overhead
- Verify layer caching works identically

## Migration Considerations

### Breaking Changes

**Removed:**
- `peelbox frontend` command
- `--context-name` flag

**Added:**
- `peelbox build` command
- Connection auto-detection

### User Impact

**Low Impact:**
- Most users likely use ad-hoc `peelbox detect | jq` workflows (not `frontend`)
- CI/CD pipelines easy to update (1 command replacement)
- Improved UX outweighs migration cost

**High Value:**
- Eliminates buildctl dependency
- 3-step workflow → 1-step workflow
- Real-time feedback improves debugging

### Rollout Plan

1. **Pre-release (v0.4.0-rc.1):**
   - Publish release candidate with new `build` command
   - Keep `frontend` with deprecation warning
   - Gather feedback from early adopters

2. **Release (v0.4.0):**
   - Remove `frontend` command
   - Update all documentation
   - Publish migration guide

3. **Post-release:**
   - Monitor issues for connection problems
   - Provide quick fixes for common migration issues
   - Add troubleshooting FAQ

## Future Enhancements (Out of Scope)

- **Remote builders**: Connect to remote BuildKit clusters
- **Distributed caching**: Shared cache across machines
- **Build optimization**: Analyze and suggest layer optimizations
- **Custom exporters**: Plugin system for custom output types
- **Parallel builds**: Build multiple services from monorepo simultaneously

These are explicitly deferred to future versions to keep this change focused.
