# Change: Direct BuildKit Daemon Interaction

## Why

Current limitations with BuildKit frontend protocol:

1. **Poor User Experience**: Users must manually pipe `peelbox frontend` output to `buildctl`, requiring buildctl installation and multi-step workflows
2. **No Build Progress**: Frontend protocol only outputs static LLB protobuf - no real-time build logs, layer caching status, or progress indicators
3. **Limited Control**: Cannot pass runtime arguments (entrypoint override), push directly to registries, or control build outputs without external buildctl invocation
4. **Workflow Friction**: Common tasks require complex multi-step commands:
   ```bash
   # Current: 3-step workflow
   peelbox detect . > spec.json
   peelbox frontend --spec spec.json > llb.pb
   cat llb.pb | buildctl --addr tcp://127.0.0.1:1234 build --local context=. --output type=docker,name=app:latest | docker load

   # Desired: 2-step workflow
   peelbox detect . > universalbuild.json
   peelbox build --spec universalbuild.json --tag app:latest
   ```
5. **No Docker Daemon Support**: Only works with standalone BuildKit, not Docker's built-in BuildKit (Docker 23.0+)

## What Changes

**BREAKING CHANGES:**
- **Remove `peelbox frontend` command entirely** - users must migrate to new `peelbox build` command
- **Remove `--context-name` flag** - no longer needed with direct daemon communication

**New Capabilities:**
1. **New `peelbox build` command** - Build from UniversalBuild spec:
   - Requires `--spec universalbuild.json` (no auto-detection)
   - Streams build progress and logs in real-time
   - Supports Docker export and OCI tar export (no registry push initially)
   - Allows entrypoint override at build time
   - Generates SBOM and provenance by default (mandatory)

2. **BuildKit gRPC Client** - Direct daemon communication:
   - Unix socket connection (default: `unix:///run/buildkit/buildkitd.sock`)
   - TCP with optional TLS (`tcp://host:port`)
   - Docker container BuildKit (`docker-container://buildkitd`)
   - Docker daemon BuildKit auto-detection (Docker 23.0+)
   - Connection pooling and health checks
   - BuildKit version validation (require v0.11.0+)

3. **Docker Daemon Fallback** - Transparent fallback when BuildKit unavailable:
   - Auto-detect Docker daemon with BuildKit support
   - Use Docker's built-in BuildKit if standalone daemon not found
   - Clear error messages if neither available

4. **Output Control** (Docker and OCI tar only):
   - `--output type=docker` - Load into Docker daemon (default)
   - `--output type=oci,dest=app.tar` - Export as OCI tarball
   - Progress bars, layer cache status, live build logs
   - Note: Registry push support deferred to future version

## Impact

**Affected Specs:**
- `buildkit-frontend` - Complete rewrite: remove frontend protocol, add gRPC client
- `output-formats` - Add build command output options

**Affected Code:**
- **Removed**: `src/buildkit/llb.rs::write_definition()` (frontend stdout protocol)
- **Removed**: `src/cli/commands.rs::FrontendArgs`, `src/main.rs::handle_frontend()`
- **Added**: `src/buildkit/client.rs` - gRPC client with tonic + buildkit-proto
- **Added**: `src/buildkit/session.rs` - Session protocol and FileSync implementation
- **Added**: `src/buildkit/filesync.rs` - File transfer protocol (fsutil packets)
- **Added**: `src/buildkit/connection.rs` - Connection pooling and health checks
- **Added**: `src/cli/build.rs` - Build command implementation
- **Modified**: `src/buildkit/llb.rs` - Keep LLB generation, remove stdout writing
- **Modified**: `src/cli/commands.rs` - Remove Frontend, add Build command
- **Modified**: `Cargo.toml` - Add `tonic`, `prost`, `buildkit-proto` dependencies

**Breaking Changes:**
1. **Frontend command removal** - Users with `peelbox frontend` in scripts must migrate to `peelbox build`
2. **CLI flag changes** - `--context-name` removed (no longer needed)
3. **New dependencies** - Requires `buildkit-client` or `tonic` crate (increases binary size by ~2MB)

**Migration Path:**
```bash
# Old workflow (BREAKS)
peelbox frontend --spec spec.json | buildctl build --local context=.

# New workflow
peelbox build --spec spec.json --tag myapp:latest
```

## Dependencies

**External Crates:**
- `tonic = "0.12"` - gRPC client framework
- `prost = "0.13"` - Protobuf serialization (already in tree via buildkit-llb)
- `buildkit-proto` - BuildKit protobuf definitions (already in tree via buildkit-llb)
- `tokio-stream` - Async stream utilities for bidirectional gRPC

**Daemon Requirements:**
- BuildKit v0.11.0+ (for SBOM/provenance support)
- Docker 23.0+ OR standalone buildkitd
- Unix socket or TCP connectivity

**File Transfer Protocol Complexity:**
- Implement BuildKit's FileSync protocol (bidirectional gRPC streaming)
- Handle fsutil.types.Packet for file stats and content
- Support diff-based transfer (only send changed files)
- Selective file transfer (only files needed by LLB operations)
- Requires understanding of BuildKit session protocol internals

## Migration Notes

**For Users:**
1. Replace `peelbox frontend | buildctl` workflow with `peelbox build --spec`
2. Keep detection step separate: `peelbox detect . > universalbuild.json`
3. Remove buildctl dependency if only used for peelbox

**For CI/CD:**
```yaml
# Old CI workflow (3 steps)
- run: peelbox detect . > spec.json
- run: peelbox frontend --spec spec.json > llb.pb
- run: cat llb.pb | buildctl build --local context=. --output type=docker | docker load

# New CI workflow (2 steps)
- run: peelbox detect . > universalbuild.json
- run: peelbox build --spec universalbuild.json --tag $IMAGE_NAME
```

## Success Criteria

1. **Workflow Simplification**: 2-step workflow (detect → build) replaces 3-step pipeline (detect → frontend → buildctl)
2. **Real-time Progress**: Users see build progress, layer caching, and logs during build
3. **Output Formats**: Docker daemon export and OCI tar export both working
4. **Docker Integration**: Auto-detects and uses Docker daemon's BuildKit when standalone unavailable
5. **File Transfer**: Efficient context transfer using BuildKit FileSync protocol
6. **Performance**: Build times equivalent or faster than buildctl (no performance regression)
7. **Breaking Change Handling**: Clear error messages for users attempting `peelbox frontend`
