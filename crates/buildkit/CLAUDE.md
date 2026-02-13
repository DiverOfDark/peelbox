# peelbox-buildkit

Native Rust gRPC client for BuildKit. Handles connection management, LLB graph construction, build session orchestration, file sync/send, progress tracking, and OCI caching.

## Module Structure

```
src/
├── lib.rs                 # Public API re-exports
├── proto.rs               # Auto-generated protobuf wrappers
├── build.rs               # Proto download & compilation (build script)
├── connection.rs          # BuildKitAddr, BuildKitConnection (Unix/TCP/Docker)
├── session.rs             # BuildSession, BuildResult, AttestationConfig, CacheImport/Export
├── llb/
│   ├── builder.rs         # LLBBuilder (Op/digest management, LLB graph construction)
│   └── strategy.rs        # BuildStrategy trait, PeelboxStrategy (Wolfi-based)
├── filesync.rs            # FileSync (directory scanning with gitignore), FileStat
├── filesync_service.rs    # FileSyncService (sends build context to BuildKit)
├── filesend_service.rs    # FileSendService (receives built image tar)
├── auth_service.rs        # AuthService (registry auth -- anonymous/no-op)
├── health_service.rs      # HealthService (session liveness -- always SERVING)
├── content_service.rs     # ContentService (OCI cache store with GC)
├── stream_conn.rs         # StreamConn (gRPC stream <-> AsyncRead/AsyncWrite adapter)
├── digest.rs              # SHA256 digest parsing and blob path resolution
├── docker.rs              # Docker daemon BuildKit endpoint detection
├── progress.rs            # ProgressTracker, ProgressEvent
├── oci_index.rs           # OciIndex, OciDescriptor (lockfile-based updates)
├── fsutil.rs              # Go FileMode bit constants
└── call_tracker.rs        # CallTracker (atomic gRPC call ID generator)
```

## Key APIs

| Type | Purpose |
|------|---------|
| `BuildKitConnection` | Connect to BuildKit daemon (auto-detect, Unix, TCP, Docker) |
| `BuildSession` | Orchestrate full build lifecycle |
| `LLBBuilder` | Construct LLB operation graphs (exec, local source, image source) |
| `BuildStrategy` trait | Pluggable build strategies |
| `PeelboxStrategy` | Default Wolfi-based multi-stage strategy |
| `ProgressTracker` | Real-time build progress tracking and logging |

## Connection Types

`BuildKitAddr` supports:
- `unix:///path/to/buildkitd.sock` -- Unix socket (default)
- `tcp://host:port` -- TCP connection
- `docker://` -- Docker daemon native (API 1.41+)
- `docker-container://container-name` -- BuildKit in container

## Session Protocol

BuildKit sessions require specific gRPC metadata headers:
- `x-docker-expose-session-uuid`, `x-docker-expose-session-name`, `x-docker-expose-session-sharedkey`
- `x-docker-expose-session-grpc-method` -- lists exposed gRPC methods

Health checks every 5 seconds -- two consecutive failures close the session.

## Critical Constraints

### Go FileMode Bit Conversion
Unix file modes must use Go's `os.FileMode` format:
- Directory: `0x80000000 | perms` (bit 31)
- Symlink: `0x08000000 | perms` (bit 27)
- Regular: just `perms`

### 3MB Chunk Limit
`StreamConn` enforces max 3MB chunks for `BytesMessage`. Larger chunks cause BuildKit errors. Defined as `MAX_CHUNK_SIZE`.

### Normalized Metadata
`FileSync` sets `uid=0, gid=0, mod_time=0` for deterministic digests -- enables cache hits across machines.

### Tar Export Timeout
5 minutes (`TAR_EXPORT_TIMEOUT_SECS = 300`). Long builds may timeout during export.

## Caching

- `CacheImport`/`CacheExport` with type string + attrs HashMap
- Supports: local, registry, gha (GitHub Actions), s3, azblob, inline
- `OciIndex` manages index.json with file locking (`fs2::FileExt`)
- GC traverses manifests -> config -> layers, deletes unreachable blobs

## Proto Compilation

`build.rs` downloads protos from BuildKit GitHub (v0.12.5 & v0.13.0). Falls back to cached protos if files exist. Import paths are rewritten and vtproto options removed for Rust compatibility.

## Dependencies

Core: `tonic` (gRPC), `prost` (protobuf), `tokio` (async), `bollard` (Docker API), `sha2` (hashing), `ignore` (gitignore), `walkdir` (traversal)

## Tests

Run with: `cargo test -p peelbox-buildkit`

Container tests require a running BuildKit daemon -- they will fail without one. Unit tests cover connection parsing, digest handling, file scanning, progress tracking, and session creation.
