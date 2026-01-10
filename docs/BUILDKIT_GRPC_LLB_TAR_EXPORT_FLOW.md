# BuildKit gRPC Communication Pattern: LLB to Tar Image Export

This document provides a comprehensive explanation of the expected communication pattern and order of operations for using BuildKit's gRPC API to build an image using LLB (Low-Level Build) definitions and export it to a tar archive.

## Table of Contents

1. [Overview](#overview)
2. [Architecture Components](#architecture-components)
3. [gRPC API Definitions](#grpc-api-definitions)
4. [LLB Definition Structure](#llb-definition-structure)
5. [Communication Flow](#communication-flow)
6. [Detailed Order of Operations](#detailed-order-of-operations)
7. [Session Management](#session-management)
8. [Export Mechanisms](#export-mechanisms)
9. [Example Flow](#example-flow)
10. [FileSync Protocol Deep Dive](#filesync-protocol-deep-dive)
11. [Local Context Upload Mechanism](#local-context-upload-mechanism)
12. [Tar Export Return Path](#tar-export-return-path)
13. [Client Interface Requirements](#client-interface-requirements)
14. [OCI/Container Image Export Mechanism](#ocicontainer-image-export-mechanism)
15. [Practical Guide: Creating docker load Compatible Tar Archives](#practical-guide-creating-docker-load-compatible-tar-archives)
16. [Conclusion](#conclusion)

---

## Overview

BuildKit uses a client-server architecture where clients communicate with the buildkitd daemon via gRPC. The build process involves:

1. **Client** prepares an LLB definition (DAG of operations)
2. **Client** establishes a session with buildkitd
3. **Client** sends a Solve request via gRPC
4. **Server** executes the build according to the LLB DAG
5. **Server** exports the result to the specified format (tar in this case)
6. **Client** receives the exported tar via session file sync

---

## Architecture Components

### Key Proto Files

- **`api/services/control/control.proto`**: Main Control service definition
- **`solver/pb/ops.proto`**: LLB operation definitions
- **`frontend/gateway/pb/gateway.proto`**: Frontend gateway interface
- **Session protos**: Various session management protocols

### Key Implementation Files

- **Client-side**:
  - `cmd/buildctl/build.go`: CLI implementation
  - `client/solve.go`: Core client solve logic
  - `client/llb/`: LLB builder API

- **Server-side**:
  - `control/control.go`: Control service implementation
  - `solver/llbsolver/solver.go`: LLB solver
  - `exporter/tar/export.go`: Tar exporter
  - `session/`: Session management

---

## gRPC API Definitions

### Control Service (Main API)

```protobuf
service Control {
    rpc Solve(SolveRequest) returns (SolveResponse);
    rpc Status(StatusRequest) returns (stream StatusResponse);
    rpc Session(stream BytesMessage) returns (stream BytesMessage);
    // ... other methods
}
```

### SolveRequest Structure

```protobuf
message SolveRequest {
    string Ref = 1;                           // Unique build reference ID
    pb.Definition Definition = 2;             // LLB definition (DAG)
    string Session = 5;                       // Session ID
    string Frontend = 6;                      // Frontend name (empty for direct LLB)
    map<string, string> FrontendAttrs = 7;   // Frontend options
    CacheOptions Cache = 8;                   // Cache import/export
    repeated string Entitlements = 9;         // Allowed capabilities
    map<string, pb.Definition> FrontendInputs = 10;
    repeated Exporter Exporters = 13;         // Output exporters
    bool EnableSessionExporter = 14;
    sourcepolicy.Policy SourcePolicy = 12;
}
```

### Exporter Configuration

```protobuf
message Exporter {
    string Type = 1;                          // "tar", "image", "local", "oci"
    map<string, string> Attrs = 2;           // Exporter-specific attributes
}
```

For tar export:
- Type: `"tar"`
- Attrs: Can be empty or include options like epoch timestamps

---

## LLB Definition Structure

### Definition Message

```protobuf
message Definition {
    repeated bytes def = 1;                   // Marshaled Op messages
    map<string, OpMetadata> metadata = 2;    // Per-vertex metadata
    Source Source = 3;                        // Source mapping info
}
```

### Op (Vertex) Types

Each vertex in the DAG is represented by an `Op`:

```protobuf
message Op {
    repeated Input inputs = 1;                // Edges to other vertices
    oneof op {
        ExecOp exec = 2;                      // Execute command
        SourceOp source = 3;                  // Import source
        FileOp file = 4;                      // File operations
        BuildOp build = 5;                    // Nested build
        MergeOp merge = 6;                    // Merge inputs
        DiffOp diff = 7;                      // Compute diff
    }
    Platform platform = 10;
    WorkerConstraints constraints = 11;
}
```

### Operation Types Explained

1. **SourceOp**: Import external sources
   - Docker images: `docker-image://alpine:latest`
   - Git repos: `git://github.com/user/repo#branch`
   - Local context: `local://context`
   - HTTP: `https://example.com/file.tar.gz`

2. **ExecOp**: Execute commands in a container
   - Mounts: input filesystems, caches, secrets, tmpfs
   - Meta: args, env, cwd, user
   - Network mode: none, host, sandbox
   - Security mode: sandbox, insecure

3. **FileOp**: Perform file operations
   - Copy files between layers
   - Create/modify files
   - Create directories
   - Remove files
   - Create symlinks

4. **MergeOp**: Merge multiple filesystem layers

5. **DiffOp**: Compute difference between two layers

---

## Communication Flow

### High-Level Sequence

```
Client                          BuildKitD Server
  |                                  |
  |-- 1. Session() [stream] -------->|
  |                                  |  Session established
  |<------- Session Messages --------|
  |                                  |
  |-- 2. Solve(SolveRequest) ------->|
  |                                  |  - Parse LLB Definition
  |                                  |  - Build DAG
  |                                  |  - Execute operations
  |                                  |  - Export to tar
  |                                  |
  |-- 3. Status(StatusRequest) ----->|  (concurrent with Solve)
  |<----- StatusResponse [stream] ---|  Progress updates
  |                                  |
  |<----- Tar data via Session ------|  File sync protocol
  |                                  |
  |<-- SolveResponse -----------------|  Export complete
  |                                  |
```

### Concurrent Operations

During a build, three parallel gRPC streams typically run:

1. **Session stream**: Bidirectional, handles:
   - Auth for registries
   - Secret provision
   - SSH agent forwarding
   - File sync (local context upload, tar download)

2. **Solve RPC**: Single request/response for build execution

3. **Status stream**: Server-to-client, provides real-time progress

---

## Detailed Order of Operations

### Phase 1: Client Preparation

**Location**: `cmd/buildctl/build.go:161-337`, `client/solve.go:92-234`

1. **Parse command-line arguments**
   - Exporter type and attributes
   - Local context directories
   - Cache import/export options
   - Secrets and SSH configurations

2. **Create or read LLB Definition**
   - If using frontend: `Frontend` field set, `Definition` is nil
   - If using direct LLB: `Definition` field contains marshaled DAG

3. **Generate unique build reference**
   ```go
   ref := identity.NewID()  // Random UUID
   ```

4. **Set up session attachables**
   ```go
   attachable := []session.Attachable{
       authprovider.NewDockerAuthProvider(...),
       sshprovider.NewSSHAgentProvider(...),
       secretprovider.New(...),
   }
   ```

5. **Create session**
   ```go
   s, err := session.NewSession(ctx, sharedKey)
   ```

6. **Configure file sync for tar output**
   ```go
   syncTargets := []filesync.FSSyncTarget{
       filesync.WithFSSync(exporterID, outputCallback)
   }
   s.Allow(filesync.NewFSSyncTarget(syncTargets...))
   ```

### Phase 2: Session Establishment

**Location**: `client/solve.go:227-233`, `session/grpc.go`

1. **Client initiates Session RPC**
   ```go
   eg.Go(func() error {
       return s.Run(ctx, grpchijack.Dialer(c.ControlClient()))
   })
   ```

2. **Session handshake**
   - Client sends session capabilities
   - Server registers session with session manager
   - Bidirectional stream established

3. **Attachables registered**
   - Auth provider ready for registry pulls
   - File sync ready for context upload and tar download

### Phase 3: Solve Request

**Location**: `client/solve.go:244-327`, `control/control.go:380-551`

**Client sends SolveRequest**:

```go
sopt := &controlapi.SolveRequest{
    Ref:           ref,
    Definition:    pbd,              // Marshaled LLB
    Exporters:     exports,          // [{Type: "tar", Attrs: {...}}]
    Session:       s.ID(),
    Frontend:      opt.Frontend,     // Empty for direct LLB
    FrontendAttrs: frontendAttrs,
    Cache:         &cacheOpt.options,
    Entitlements:  opt.AllowedEntitlements,
    SourcePolicy:  opt.SourcePolicy,
}
resp, err := c.ControlClient().Solve(ctx, sopt)
```

**Server receives and processes**:

1. **Validate request** (`control/control.go:380-389`)
   ```go
   translateLegacySolveRequest(req)  // Handle deprecated fields
   ```

2. **Get worker** (`control/control.go:397-400`)
   ```go
   w, err := c.opt.WorkerController.GetDefault()
   ```

3. **Resolve exporters** (`control/control.go:414-426`)
   ```go
   for i, ex := range req.Exporters {
       exp, err := w.Exporter(ex.Type, sessionManager)
       expi, err := exp.Resolve(ctx, i, ex.Attrs)
       expis = append(expis, expi)
   }
   ```

4. **Invoke solver** (`control/control.go:534-544`)
   ```go
   resp, err := c.solver.Solve(ctx, req.Ref, req.Session,
       frontend.SolveRequest{...},
       llbsolver.ExporterRequest{Exporters: expis, ...},
       entitlements, processors, internal, sourcePolicy, ...)
   ```

### Phase 4: LLB Execution

**Location**: `solver/llbsolver/solver.go`, `solver/solver.go`

1. **Parse LLB Definition**
   - Unmarshal each `Op` from `def.Def[]`
   - Build dependency graph from `inputs`
   - Calculate digest for each vertex

2. **Topological sort**
   - Determine execution order respecting dependencies
   - Identify parallelizable operations

3. **Execute operations** (in dependency order)

   For **SourceOp**:
   ```
   - Resolve source identifier
   - Pull docker image / clone git / read local
   - Create snapshot (cached layer)
   ```

   For **ExecOp**:
   ```
   - Prepare mounts from input refs
   - Create container with executor (runc/containerd)
   - Run command with specified meta (args, env, cwd, user)
   - Capture output snapshot
   - Cache result
   ```

   For **FileOp**:
   ```
   - Mount input layers
   - Apply file actions (copy, mkdir, mkfile, rm)
   - Create output snapshot
   ```

4. **Build result**
   - Final vertex output becomes build result
   - Store as cache ref (immutable snapshot)

### Phase 5: Export to Tar

**Location**: `exporter/tar/export.go`, `session/filesync/`

1. **Tar exporter receives result** (`exporter/tar/export.go:80-185`)
   ```go
   func (e *localExporterInstance) Export(
       ctx context.Context,
       inp *exporter.Source,       // Build result ref
       buildInfo exporter.ExportBuildInfo,
   ) (map[string]string, exporter.DescriptorReference, error)
   ```

2. **Create filesystem from result**
   ```go
   outputFS, cleanup, err := local.CreateFS(
       ctx, buildInfo.SessionID, k, ref, attestations, now, isMap, e.opts
   )
   ```

3. **Get session caller**
   ```go
   caller, err := e.opt.SessionManager.Get(ctx, buildInfo.SessionID, false)
   ```

4. **Initiate file sync writer**
   ```go
   w, err := filesync.CopyFileWriter(ctx, nil, e.id, caller)
   ```

5. **Write tar to session stream** (`exporter/tar/export_unix.go`)
   ```go
   err := writeTar(ctx, fs, w)
   ```
   - Walks filesystem tree
   - Creates tar headers
   - Streams tar data to session
   - Client receives via session file sync protocol

6. **Client saves tar**
   - File sync target receives stream
   - Writes to output file or callback

### Phase 6: Status Updates

**Location**: `client/solve.go:351-379`, `control/control.go:553-584`

Running concurrently with solve:

**Client initiates Status stream**:
```go
stream, err := c.ControlClient().Status(ctx, &controlapi.StatusRequest{
    Ref: ref,
})
for {
    resp, err := stream.Recv()
    if statusChan != nil {
        statusChan <- NewSolveStatus(resp)
    }
}
```

**Server sends updates**:
```go
ch := make(chan *client.SolveStatus, 8)
c.solver.Status(ctx, req.Ref, ch)
for ss := range ch {
    for _, sr := range ss.Marshal() {
        stream.SendMsg(sr)
    }
}
```

**Status messages include**:
- Vertex (operation) start/complete
- Progress for downloads, builds, exports
- Logs from ExecOp
- Warnings and errors

### Phase 7: Completion

1. **Server completes Solve RPC**
   ```go
   return &controlapi.SolveResponse{
       ExporterResponse: resp.ExporterResponse,
   }
   ```

2. **Client receives response**
   - ExporterResponse contains metadata (e.g., image digest)
   - Tar already received via session file sync
   - Status stream completes

3. **Cleanup**
   - Session closes
   - Temporary resources released
   - Build reference can be used for history

---

## Session Management

### Session Protocol

Sessions use gRPC stream hijacking for multiplexed communication:

```
Session Stream (bidirectional)
    ├─ Auth requests/responses
    ├─ Secret requests/responses
    ├─ SSH forwarding
    └─ File sync
        ├─ DiffCopy (client → server): Local context upload
        └─ CopyFileWriter (server → client): Tar download
```

### File Sync Protocol

**Location**: `session/filesync/filesync.proto`

For tar export, the server acts as sender:

1. Server calls `filesync.CopyFileWriter(ctx, nil, exporterID, caller)`
2. This initiates a file sync stream to client
3. Server writes tar bytes to the writer
4. Client's file sync target receives and processes the data
5. Client callback writes to output file or memory

---

## Export Mechanisms

### Exporter Types

BuildKit supports multiple exporters, configured via `Exporters` field:

1. **tar** (`exporter/tar/export.go`):
   - Type: `"tar"`
   - Output: Tar archive via session
   - Use case: Save as tar file

2. **image** (`exporter/containerimage/export.go`):
   - Type: `"image"`
   - Output: OCI/Docker image
   - Options: push, store in containerd, name

3. **local** (`exporter/local/export.go`):
   - Type: `"local"`
   - Output: Directory via session
   - Use case: Extract to filesystem

4. **oci** (`exporter/oci/export.go`):
   - Type: `"oci"`
   - Output: OCI image layout
   - Use case: OCI-compatible tar

### Tar Export Specifics

**Exporter Attributes**:
```go
Attrs: map[string]string{
    "epoch": "1234567890",  // Optional: Set file timestamps
}
```

**Export Flow**:
1. Exporter receives immutable cache ref (final build result)
2. Mounts ref as read-only filesystem
3. Walks filesystem tree
4. Creates tar with proper headers (permissions, ownership, timestamps)
5. Streams tar via session file sync
6. Client receives and writes to destination

**Timestamp Handling**:
- If `epoch` set: All files get this timestamp
- If SOURCE_DATE_EPOCH in frontend attrs: Propagated to exporter
- Otherwise: Uses current time truncated to seconds

---

## Example Flow

### Scenario: Build Alpine Image and Export to Tar

**Client Code** (simplified from `cmd/buildctl/build.go`):

```go
// 1. Create LLB definition
state := llb.Image("docker.io/library/alpine:latest")
state = state.Run(llb.Shlex("apk add curl")).Root()

def, err := state.Marshal(ctx)

// 2. Configure exporter
exports := []client.ExportEntry{
    {
        Type:   client.ExporterTar,
        Output: outputCallback,  // Receives tar bytes
    },
}

// 3. Create session with file sync
session, err := session.NewSession(ctx, "")
session.Allow(filesync.NewFSSyncTarget(
    filesync.WithFSSync(0, outputCallback),
))

// 4. Start session in background
go session.Run(ctx, grpchijack.Dialer(client))

// 5. Send Solve request
solveOpt := client.SolveOpt{
    Exports: exports,
    Session: []session.Attachable{authProvider},
}
resp, err := client.Solve(ctx, def, solveOpt, statusChan)
```

**LLB Definition** (marshaled):

```
Definition {
    def: [
        Op {  // Digest: sha256:abc123...
            op: SourceOp {
                identifier: "docker-image://docker.io/library/alpine:latest"
            }
        },
        Op {  // Digest: sha256:def456...
            inputs: [Input{digest: "sha256:abc123...", index: 0}]
            op: ExecOp {
                meta: Meta {
                    args: ["/bin/sh", "-c", "apk add curl"]
                }
                mounts: [
                    Mount {input: 0, dest: "/", output: 0}
                ]
            }
        }
    ]
    metadata: {
        "sha256:abc123...": OpMetadata {...}
        "sha256:def456...": OpMetadata {...}
    }
}
```

**gRPC Messages**:

1. **Session establishment**:
   ```
   Client → Server: Session() [stream start]
   Client ← Server: Session capabilities
   ```

2. **Solve request**:
   ```protobuf
   Client → Server: SolveRequest {
       Ref: "abc123-def456-..."
       Definition: [marshaled LLB above]
       Exporters: [{Type: "tar"}]
       Session: "session-xyz"
   }
   ```

3. **Status updates** (concurrent):
   ```protobuf
   Client → Server: StatusRequest {Ref: "abc123-def456-..."}

   Client ← Server: StatusResponse {
       vertexes: [
           Vertex {digest: "sha256:abc123...", name: "docker.io/library/alpine:latest"},
           Vertex {digest: "sha256:def456...", name: "RUN apk add curl"}
       ]
   }
   ```

4. **Execution on server**:
   ```
   - Pull alpine:latest image
   - Create container with alpine rootfs
   - Execute: /bin/sh -c "apk add curl"
   - Capture resulting filesystem snapshot
   ```

5. **Export**:
   ```
   - Mount final snapshot
   - Walk filesystem: /, /bin, /etc, /usr, ...
   - Create tar headers
   - Stream tar via session file sync
   ```

6. **Client receives tar**:
   ```go
   outputCallback(name string, stat *fsutil.Stat) error {
       // Receives tar stream
       // Writes to output file
   }
   ```

7. **Solve response**:
   ```protobuf
   Client ← Server: SolveResponse {
       ExporterResponse: {}
   }
   ```

---

## Key Data Structures

### LLB Definition Marshaling

**Client-side** (`client/llb/marshal.go`):

```go
func (s State) Marshal(ctx context.Context, co ...ConstraintsOpt) (*Definition, error) {
    def := &Definition{
        Def:      make([][]byte, 0),
        Metadata: make(map[digest.Digest]OpMetadata),
    }

    // Recursive traversal of state graph
    // Each vertex marshaled to protobuf Op
    // Stored in def.Def with digest as key
}
```

**Server-side** (`client/llb/definition.go:31-111`):

```go
func NewDefinitionOp(def *pb.Definition) (*DefinitionOp, error) {
    ops := make(map[digest.Digest]*pb.Op)
    for _, dt := range def.Def {
        var op pb.Op
        proto.Unmarshal(dt, &op)
        dgst := digest.FromBytes(dt)
        ops[dgst] = &op
    }
    // Returns graph structure for solver
}
```

### Cache References

Each operation result is stored as an **immutable cache reference**:

```go
type ImmutableRef interface {
    ID() string
    Mountable(ctx context.Context) (snapshot.Mountable, error)
    Size(ctx context.Context) (int64, error)
    // ...
}
```

These refs are content-addressed by:
- Input refs
- Operation type and parameters
- Allows caching and reuse

### Exporter Source

```go
type Source struct {
    Ref         cache.ImmutableRef         // Single ref (default)
    Refs        map[string]cache.ImmutableRef  // Multi-platform
    Metadata    map[string][]byte
    Attestations map[string][]Attestation
}
```

---

## Error Handling

### Client-side Errors

- **Connection failures**: gRPC connection to buildkitd
- **Session errors**: Auth failures, secret not found
- **LLB validation**: Invalid operation graph
- **Build failures**: Propagated from server via Status stream

### Server-side Errors

- **Parse errors**: Invalid LLB definition
- **Execution errors**: Command failed, network unreachable
- **Export errors**: Filesystem access, session disconnected
- **Cache errors**: Snapshot corruption, disk full

All errors propagate through:
1. Status stream (real-time)
2. Solve response (final error)
3. gRPC status codes

---

## Performance Considerations

### Parallelism

- **Independent operations**: Executed concurrently by solver
- **Layer pulls**: Parallel image layer downloads
- **Mounts**: Lazy mounting with content-addressable snapshots

### Caching

- **Local cache**: Previous build results reused by digest
- **Remote cache**: Registry or custom backends
- **Inline cache**: Embedded in exported images

### Streaming

- **File sync**: Streams data without buffering entire tar
- **Status updates**: Incremental progress reporting
- **Logs**: Streamed from ExecOp in real-time

---

## Security

### Entitlements

Required for privileged operations:

```go
Entitlements: []string{
    "network.host",      // Host networking
    "security.insecure", // Privileged mode
}
```

### Source Policy

Restrict allowed sources:

```protobuf
SourcePolicy: &sourcepolicy.Policy{
    Rules: [
        {Selector: {Identifier: "docker-image://docker.io/*"}, Action: ALLOW},
        {Selector: {Identifier: "git://*"}, Action: DENY},
    ]
}
```

### Secrets

Secure secret injection:

```go
// Client provides secret via session
secretProvider := secretprovider.NewStore(secrets)
session.Allow(secretProvider)

// LLB references secret
mount := llb.Secret("my-secret", llb.SecretAsEnv(true))
```

---

## FileSync Protocol Deep Dive

The FileSync protocol is the foundation for bidirectional file transfer between client and server over the session stream.

### Protocol Definition

**Location**: `session/filesync/filesync.proto`

```protobuf
// FileSync exposes local files from the client to the server.
service FileSync {
    rpc DiffCopy(stream fsutil.types.Packet) returns (stream fsutil.types.Packet);
    rpc TarStream(stream fsutil.types.Packet) returns (stream fsutil.types.Packet);
}

// FileSend allows sending files from the server back to the client.
service FileSend {
    rpc DiffCopy(stream BytesMessage) returns (stream BytesMessage);
}

message BytesMessage {
    bytes data = 1;
}
```

### Two Directions of File Transfer

1. **Client → Server (FileSync.DiffCopy)**:
   - Used for uploading local context
   - Client acts as FileSync server
   - Server calls client's FileSync methods
   - Uses fsutil.Packet stream (metadata + content)

2. **Server → Client (FileSend.DiffCopy)**:
   - Used for tar export, local export
   - Client acts as FileSend server
   - Server calls client's FileSend methods
   - Uses BytesMessage stream (raw bytes)

### FileSync Architecture

```
Session Stream (bidirectional gRPC)
    │
    ├── FileSync Service (Client implements, Server calls)
    │   └── DiffCopy: Client → Server file transfer
    │       └── Used by: SourceOp with local:// identifier
    │
    └── FileSend Service (Client implements, Server calls)
        └── DiffCopy: Server → Client file transfer
            └── Used by: tar, local, oci exporters
```

### Session Attachables

Both client and server register "attachables" to the session:

**Client-side** (`client/solve.go:138-221`):

```go
// 1. Provider for sending files TO server
s.Allow(filesync.NewFSSyncProvider(syncedDirs))

// 2. Target for receiving files FROM server
syncTargets := []filesync.FSSyncTarget{
    filesync.WithFSSync(exporterID, outputCallback)
}
s.Allow(filesync.NewFSSyncTarget(syncTargets...))
```

**Implementation**:
- `FSSyncProvider` implements `FileSync` gRPC service
- `SyncTarget` implements `FileSend` gRPC service
- Both registered on client's session gRPC server

### DiffCopy Protocol Details

**Packet-based transfer** (Client → Server):

```go
// Uses fsutil.Packet which includes:
type Packet struct {
    Type PacketType  // PACKET_STAT, PACKET_DATA, PACKET_FIN
    Stat *Stat       // File metadata
    Data []byte      // File content
}
```

**BytesMessage transfer** (Server → Client):

```go
// Simple byte chunks with 3MB max size
message BytesMessage {
    bytes data = 1;  // Up to 3MB per message
}
```

### Metadata Transmission

Metadata passed via gRPC metadata headers:

```go
// Encoded in request metadata
metadata := map[string][]string{
    "dir-name":          {localName},         // Which local dir
    "include-patterns":  {patterns...},       // Include filters
    "exclude-patterns":  {patterns...},       // Exclude filters
    "followpaths":       {paths...},          // Symlink following
    "exporter-md-*":     {exporterMetadata},  // Custom metadata
}
```

### Stream Chunking

**Location**: `session/filesync/diffcopy.go:45-83`

```go
func (wc *streamWriterCloser) Write(dt []byte) (int, error) {
    const maxChunkSize = 3 * 1024 * 1024  // 3MB limit

    // Split large writes into chunks
    if len(dt) > maxChunkSize {
        n1, err := wc.Write(dt[:maxChunkSize])
        n2, err := wc.Write(dt[maxChunkSize:])
        return n1 + n2, nil
    }

    // Send as BytesMessage
    return wc.SendMsg(&BytesMessage{Data: dt})
}
```

**Why 3MB?**: gRPC default max message size is 4MB. Using 3MB leaves headroom for metadata.

---

## Local Context Upload Mechanism

When LLB contains a `local://` source, the client must upload local files to the server.

### How It Works

**1. LLB Definition with Local Source**

```go
// Client creates LLB state
state := llb.Local("context",
    llb.IncludePatterns([]string{"**/*.go"}),
    llb.ExcludePatterns([]string{"vendor/**"}),
)
```

This produces:

```protobuf
Op {
    op: SourceOp {
        identifier: "local://context"
        attrs: {
            "local.session-id": "session-xyz"
            "local.include-patterns": "[\"**/*.go\"]"
            "local.exclude-patterns": "[\"vendor/**\"]"
        }
    }
}
```

**2. Client Prepares Local Directories**

**Location**: `client/solve.go:97-104`, `client/solve.go:425-467`

```go
// Parse local mounts from SolveOpt
solveOpt.LocalMounts = map[string]fsutil.FS{
    "context": fsutil.NewFS("/path/to/context"),
}

// Extract which locals are actually used in LLB
syncedDirs, err := prepareSyncedFiles(def, mounts)
// Returns: map[string]fsutil.FS

// Register with session
s.Allow(filesync.NewFSSyncProvider(syncedDirs))
```

**3. Server Requests Local Files**

**Location**: `source/local/source.go:164-334`

When solver executes the SourceOp:

```go
func (ls *localSourceHandler) Snapshot(ctx, jobCtx) (cache.ImmutableRef, error) {
    // Get session caller
    caller, err := ls.sm.Get(ctx, sessionID, false)

    // Create mutable cache ref
    mutable, err := ls.cm.New(ctx, ...)

    // Mount it
    mount, err := mutable.Mount(ctx, false, nil)
    dest, err := lm.Mount()  // Local filesystem path

    // Request files from client via FileSync
    opt := filesync.FSSendRequestOpt{
        Name:            "context",           // Which local dir
        IncludePatterns: includePatterns,
        ExcludePatterns: excludePatterns,
        DestDir:         dest,               // Where to write
        CacheUpdater:    cacheUpdater,       // Content hash tracking
        ProgressCb:      progressCallback,
    }

    err = filesync.FSSync(ctx, caller, opt)

    // Commit to immutable ref
    snap, err := mutable.Commit(ctx)
    return snap, nil
}
```

**4. FileSync RPC Call**

**Location**: `session/filesync/filesync.go:182-248`

```go
func FSSync(ctx, caller, opt) error {
    // Choose protocol (diffcopy or tarstream)
    client := NewFileSyncClient(caller.Conn())

    // Set metadata
    opts := map[string][]string{
        "dir-name":         {opt.Name},
        "include-patterns": opt.IncludePatterns,
        "exclude-patterns": opt.ExcludePatterns,
    }
    ctx = metadata.NewOutgoingContext(ctx, opts)

    // Initiate DiffCopy stream
    stream, err := client.DiffCopy(ctx)

    // Receive files into destDir
    return recvDiffCopy(stream, opt.DestDir, cacheUpdater, ...)
}
```

**5. Client Responds with Files**

**Location**: `session/filesync/filesync.go:71-136`

Client's FileSync server receives request:

```go
func (sp *fsSyncProvider) DiffCopy(stream FileSync_DiffCopyServer) error {
    // Extract metadata
    opts, _ := metadata.FromIncomingContext(stream.Context())
    dirName := opts["dir-name"][0]

    // Lookup registered directory
    dir, ok := sp.dirs.LookupDir(dirName)

    // Apply filters
    dir, err := fsutil.NewFilterFS(dir, &fsutil.FilterOpt{
        ExcludePatterns: opts["exclude-patterns"],
        IncludePatterns: opts["include-patterns"],
    })

    // Send directory contents
    return sendDiffCopy(stream, dir, progress)
}
```

**6. File Transfer Protocol**

Uses `fsutil` library to walk and stream:

```
Client sends Packet stream:
    PACKET_STAT  {path: "/", mode: DIR, ...}
    PACKET_STAT  {path: "/main.go", mode: FILE, size: 1024, ...}
    PACKET_DATA  {data: [1024 bytes of main.go]}
    PACKET_STAT  {path: "/pkg", mode: DIR, ...}
    PACKET_STAT  {path: "/pkg/util.go", mode: FILE, size: 512, ...}
    PACKET_DATA  {data: [512 bytes of util.go]}
    PACKET_FIN   {end of transfer}
```

Server receives and writes to mounted destination.

### Content Hashing

During transfer, BuildKit calculates content hashes:

```go
type CacheUpdater interface {
    HandleChange(kind fsutil.ChangeKind, path string, fi os.FileInfo, err error) error
    ContentHasher() fsutil.ContentHasher
}
```

Hashes are used for:
- Cache key calculation
- Change detection for incremental updates
- Deduplication across builds

### Caching Local Sources

**SharedKey**: Identifies cached local snapshots

```go
sharedKey := localName + ":" + sharedKeyHint + ":" + caller.SharedKey()
// Example: "context::abc123-session-key"
```

If SharedKey exists in cache:
- Reuse existing mutable ref
- Only transfer changed files (via differ)

If not cached:
- Create new mutable ref
- Transfer all files
- Store with SharedKey

---

## Tar Export Return Path

Exporting to tar involves streaming data from server back to client.

### Export Initiation

**Location**: `exporter/tar/export.go:80-185`

```go
func (e *localExporterInstance) Export(
    ctx context.Context,
    inp *exporter.Source,        // Build result
    buildInfo exporter.ExportBuildInfo,
) (map[string]string, exporter.DescriptorReference, error) {
    // 1. Create filesystem from build result
    outputFS, cleanup, err := local.CreateFS(
        ctx, buildInfo.SessionID, "", inp.Ref, attestations, now, isMap, e.opts
    )

    // 2. Get session caller
    caller, err := e.opt.SessionManager.Get(ctx, buildInfo.SessionID, false)

    // 3. Create file writer to client
    w, err := filesync.CopyFileWriter(ctx, nil, e.id, caller)

    // 4. Write tar to stream
    err := writeTar(ctx, fs, w)

    return nil, nil, w.Close()
}
```

### CopyFileWriter

**Location**: `session/filesync/filesync.go:385-416`

```go
func CopyFileWriter(ctx, md map[string]string, id int, caller) (io.WriteCloser, error) {
    // Check if client supports FileSend.DiffCopy
    method := session.MethodURL(FileSend_ServiceDesc.ServiceName, "diffcopy")
    if !caller.Supports(method) {
        return nil, errors.Errorf("method %s not supported", method)
    }

    // Create FileSend client
    client := NewFileSendClient(caller.Conn())

    // Set metadata including exporter ID
    opts := map[string][]string{
        "buildkit-attachable-exporter-id": {fmt.Sprint(id)},
    }
    for k, v := range md {
        opts["exporter-md-"+k] = []string{v}
    }
    ctx = metadata.NewOutgoingContext(ctx, opts)

    // Initiate DiffCopy stream
    cc, err := client.DiffCopy(ctx)

    // Return write closer
    return newStreamWriter(cc), nil
}
```

### Stream Writer

**Location**: `session/filesync/diffcopy.go:24-84`

```go
type streamWriterCloser struct {
    grpc.ClientStream
}

func (wc *streamWriterCloser) Write(dt []byte) (int, error) {
    const maxChunkSize = 3 * 1024 * 1024

    // Split into chunks if needed
    // ...

    // Send as BytesMessage
    if err := wc.SendMsg(&BytesMessage{Data: dt}); err != nil {
        return 0, err
    }
    return len(dt), nil
}

func (wc *streamWriterCloser) Close() error {
    // Signal end of stream
    if err := wc.CloseSend(); err != nil {
        return err
    }

    // Block until client acknowledges
    var bm BytesMessage
    if err := wc.RecvMsg(&bm); !errors.Is(err, io.EOF) {
        return err
    }
    return nil
}
```

### Client Receives Tar

**Location**: `session/filesync/filesync.go:326-357`

Client's FileSend server:

```go
func (sp *SyncTarget) DiffCopy(stream FileSend_DiffCopyServer) error {
    // Extract exporter ID from metadata
    id := sp.chooser(stream.Context())

    // Get registered output function
    f, ok := sp.fs[id]

    // Get metadata
    opts, _ := metadata.FromIncomingContext(stream.Context())
    md := map[string]string{}
    for k, v := range opts {
        if strings.HasPrefix(k, "exporter-md-") {
            md[k[12:]] = v[0]
        }
    }

    // Create output writer
    wc, err := f(md)  // Calls user callback

    defer wc.Close()

    // Receive bytes and write
    return writeTargetFile(stream, wc)
}

func writeTargetFile(ds grpc.ServerStream, wc io.WriteCloser) error {
    var bm BytesMessage
    for {
        bm.Data = bm.Data[:0]
        if err := ds.RecvMsg(&bm); err != nil {
            if errors.Is(err, io.EOF) {
                return nil
            }
            return err
        }
        if _, err := wc.Write(bm.Data); err != nil {
            return err
        }
    }
}
```

### User Callback

**Location**: `cmd/buildctl/build.go:355-409`

User provides callback when setting up exports:

```go
exports := []client.ExportEntry{
    {
        Type: client.ExporterTar,
        Output: func(map[string]string) (io.WriteCloser, error) {
            // Return writer for tar output
            return os.Create("output.tar")
        },
    },
}
```

Or using buildctl's file sync:

```go
solveOpt.Exports = exports
resp, err := c.Build(ctx, solveOpt, "buildctl", buildFunc, statusChan)
```

### Complete Tar Export Flow

```
Server                                          Client
------                                          ------
[Build complete]
    |
    v
exporter/tar/export.go:
  CreateFS(result ref)
    |
    v
  CopyFileWriter(ctx, exporterID, caller)
    |-- FileSend.DiffCopy(ctx) -----------------> [FileSend server]
    |                                                |
    |                                                v
    |                                            Extract exporter ID
    |                                                |
    |                                                v
    |                                            Call user callback
    |                                                |
    |                                                v
    |                                            wc = os.Create("out.tar")
    |                                                |
    |<-- Ack ready to receive ---------------------|
    |                                                |
writeTar(fs, stream):                               |
  Walk filesystem                                    |
  Create tar headers                                |
  Write tar bytes                                    |
    |-- BytesMessage{data: [3MB chunk]} ----------->|
    |-- BytesMessage{data: [3MB chunk]} ----------->|-- wc.Write(chunk)
    |-- BytesMessage{data: [remaining]} ----------->|-- wc.Write(chunk)
    |                                                |
Close stream                                         |
    |-- CloseSend() ------------------------------>|
    |                                                |
    |                                                v
    |                                            wc.Close()
    |<-- EOF ----------------------------------------|
    |                                                |
    v                                                v
Complete                                          Tar saved
```

---

## Client Interface Requirements

To interact with BuildKit via gRPC, clients must implement or provide several interfaces.

### Core Interfaces

#### 1. session.Attachable

**Location**: `session/session.go:33-35`

```go
type Attachable interface {
    Register(*grpc.Server)
}
```

**Purpose**: Register services on the session's gRPC server.

**Implementations**:
- `filesync.FSSyncProvider`: Provides local files to server
- `filesync.SyncTarget`: Receives files from server
- `authprovider.DockerAuthProvider`: Registry authentication
- `secretprovider.Store`: Secret provision
- `sshprovider.SSHAgentProvider`: SSH agent forwarding

**Example**:

```go
type MyAttachable struct{}

func (a *MyAttachable) Register(server *grpc.Server) {
    // Register your gRPC services
    RegisterMyServiceServer(server, a)
}

// Usage:
session.Allow(myAttachable)
```

#### 2. session.Caller

**Location**: `session/manager.go:15-20`

```go
type Caller interface {
    Context() context.Context
    Supports(method string) bool
    Conn() *grpc.ClientConn
    SharedKey() string
}
```

**Purpose**: Represents a client session that the server can call.

**Provided by**: SessionManager.Get() - not implemented by user.

**Usage** (server-side):

```go
caller, err := sessionManager.Get(ctx, sessionID, false)

// Check capability
if caller.Supports("/moby.filesync.v1.FileSync/DiffCopy") {
    // Call client's service
    client := filesync.NewFileSyncClient(caller.Conn())
    stream, err := client.DiffCopy(ctx)
}
```

#### 3. filesync.DirSource

**Location**: `session/filesync/filesync.go:47-49`

```go
type DirSource interface {
    LookupDir(string) (fsutil.FS, bool)
}
```

**Purpose**: Provide local directories for upload.

**Implementation**:

```go
type MyDirSource struct {
    dirs map[string]string
}

func (ds *MyDirSource) LookupDir(name string) (fsutil.FS, bool) {
    path, ok := ds.dirs[name]
    if !ok {
        return nil, false
    }
    fs, err := fsutil.NewFS(path)
    if err != nil {
        return nil, false
    }
    return fs, true
}

// Usage:
provider := filesync.NewFSSyncProvider(myDirSource)
session.Allow(provider)
```

#### 4. filesync.FileOutputFunc

**Location**: `session/filesync/filesync.go:40`

```go
type FileOutputFunc func(map[string]string) (io.WriteCloser, error)
```

**Purpose**: Create writer for files coming from server.

**Implementation**:

```go
outputFunc := func(metadata map[string]string) (io.WriteCloser, error) {
    // metadata may contain exporter-specific info
    filename := metadata["filename"]
    if filename == "" {
        filename = "output.tar"
    }
    return os.Create(filename)
}

// Usage:
target := filesync.NewFSSyncTarget(
    filesync.WithFSSync(exporterID, outputFunc),
)
session.Allow(target)
```

### Required Client Setup

A complete client must:

**1. Create and configure session**:

```go
session, err := session.NewSession(ctx, sharedKey)
```

**2. Register file sync provider** (for local context):

```go
localDirs := map[string]fsutil.FS{
    "context":    fsutil.NewFS("/path/to/context"),
    "dockerfile": fsutil.NewFS("/path/to/dockerfile"),
}
session.Allow(filesync.NewFSSyncProvider(
    filesync.StaticDirSource(localDirs),
))
```

**3. Register file sync target** (for exports):

```go
outputFunc := func(md map[string]string) (io.WriteCloser, error) {
    return os.Create("output.tar")
}
session.Allow(filesync.NewFSSyncTarget(
    filesync.WithFSSync(0, outputFunc),  // exporterID = 0
))
```

**4. Register auth provider** (for pulling images):

```go
authProvider := authprovider.NewDockerAuthProvider(
    authprovider.DockerAuthProviderConfig{
        ConfigFile: dockerConfig,
    },
)
session.Allow(authProvider)
```

**5. Register secret provider** (optional):

```go
secrets := map[string][]byte{
    "my-secret": []byte("secret-value"),
}
secretProvider := secretprovider.NewStore(secrets)
session.Allow(secretProvider)
```

**6. Start session** (in background):

```go
go func() {
    err := session.Run(ctx, grpchijack.Dialer(controlClient))
    if err != nil {
        log.Errorf("session error: %v", err)
    }
}()
```

**7. Send Solve request**:

```go
solveReq := &controlapi.SolveRequest{
    Ref:        buildRef,
    Definition: llbDefinition,
    Session:    session.ID(),
    Exporters: []*controlapi.Exporter{
        {Type: "tar", Attrs: map[string]string{}},
    },
}
resp, err := controlClient.Solve(ctx, solveReq)
```

### Optional Interfaces

#### Progress Writer

**Location**: `util/progress/progresswriter/`

For status updates:

```go
pw, err := progresswriter.NewPrinter(ctx, os.Stderr, "auto")

statusChan := make(chan *client.SolveStatus)
go func() {
    for status := range statusChan {
        // Display progress
    }
}()

// Pass statusChan to Solve
```

#### Session Dialer

**Location**: `session/session.go:30`

```go
type Dialer func(ctx context.Context, proto string, meta map[string][]string) (net.Conn, error)
```

Default implementation:

```go
dialer := grpchijack.Dialer(controlClient)
```

Custom implementation for different transports:

```go
customDialer := func(ctx context.Context, proto string, meta map[string][]string) (net.Conn, error) {
    // Custom connection logic
    // e.g., HTTP/2, Unix socket, etc.
}
```

### Minimal Client Example

```go
package main

import (
    "context"
    "os"

    "github.com/moby/buildkit/client"
    "github.com/moby/buildkit/client/llb"
    "github.com/moby/buildkit/session"
    "github.com/moby/buildkit/session/auth/authprovider"
    "github.com/moby/buildkit/session/filesync"
    "github.com/moby/buildkit/session/grpchijack"
    "github.com/tonistiigi/fsutil"
    "google.golang.org/grpc"
)

func main() {
    ctx := context.Background()

    // 1. Connect to buildkitd
    conn, err := grpc.Dial("unix:///run/buildkit/buildkitd.sock",
        grpc.WithInsecure())
    if err != nil {
        panic(err)
    }
    c, err := client.New(ctx, "", client.WithContextDialer(
        func(context.Context, string) (net.Conn, error) {
            return conn, nil
        }))
    if err != nil {
        panic(err)
    }

    // 2. Create LLB
    state := llb.Image("alpine:latest").
        Run(llb.Shlex("echo hello")).Root()
    def, err := state.Marshal(ctx)
    if err != nil {
        panic(err)
    }

    // 3. Setup session
    sess, err := session.NewSession(ctx, "")
    if err != nil {
        panic(err)
    }

    // Auth for pulling alpine
    sess.Allow(authprovider.NewDockerAuthProvider(
        authprovider.DockerAuthProviderConfig{}))

    // Tar output handler
    sess.Allow(filesync.NewFSSyncTarget(
        filesync.WithFSSync(0, func(map[string]string) (io.WriteCloser, error) {
            return os.Create("output.tar")
        })))

    // Start session
    go sess.Run(ctx, grpchijack.Dialer(c.ControlClient()))

    // 4. Solve
    _, err = c.Solve(ctx, def, client.SolveOpt{
        Exports: []client.ExportEntry{
            {Type: client.ExporterTar},
        },
        Session: []session.Attachable{},
    }, nil)
    if err != nil {
        panic(err)
    }

    println("Build complete, output.tar created")
}
```

---

## OCI/Container Image Export Mechanism

BuildKit supports exporting build results as OCI or Docker images, which can be stored locally in containerd, pushed to registries, or exported as tar archives.

### Export Types

**1. Container Image (`client.ExporterImage`)**:
- Stores image in containerd image store
- Optionally pushes to registry
- Returns image digest and descriptor

**2. OCI Archive (`client.ExporterOCI`)**:
- Exports OCI Image Layout as tar
- Includes index.json, manifest, config, layers
- Compatible with OCI runtime spec

**3. Docker Archive (`client.ExporterDocker`)**:
- Exports Docker v2 image format
- Single platform only
- Compatible with `docker load`

### Image Export Flow

**Location**: `exporter/containerimage/export.go:222-384`

```go
func (e *imageExporterInstance) Export(
    ctx context.Context,
    src *exporter.Source,
    buildInfo exporter.ExportBuildInfo,
) (map[string]string, exporter.DescriptorReference, error) {
    // 1. Commit to OCI descriptor
    desc, err := e.opt.ImageWriter.Commit(ctx, src, sessionID, inlineCache, &opts)

    // 2. Store in containerd (if store=true)
    if e.opt.Images != nil && e.store {
        img := images.Image{
            Name:   targetName,
            Target: *desc,
        }
        e.opt.Images.Create(ctx, img)
    }

    // 3. Unpack for runtime (if unpack=true)
    if e.unpack {
        err := e.unpackImage(ctx, img, src, sessionGroup)
    }

    // 4. Push to registry (if push=true)
    if e.push {
        err = e.pushImage(ctx, src, sessionID, targetName, desc.Digest)
    }

    // Return image digest
    resp[exptypes.ExporterImageDigestKey] = desc.Digest.String()
    return resp, nil, nil
}
```

### ImageWriter.Commit Process

**Location**: `exporter/containerimage/writer.go:66-185`

The commit process creates an OCI image from build results:

**1. Export Layers**

```go
func (ic *ImageWriter) exportLayers(
    ctx, refCfg, sessionGroup, refs...
) ([]solver.Remote, error) {
    // For each cache ref:
    remotes, err := ref.GetRemotes(ctx, true, refCfg, false, sessionGroup)

    // Remote contains:
    // - Descriptors: Layer blob descriptors
    // - Provider: Content provider for blobs
    return remotes, nil
}
```

**What happens**:
- Each cache ref (build layer) converted to content-addressable blob
- Layers compressed (gzip, zstd, uncompressed) based on config
- Diffed against parent to create layer tarballs
- Descriptors include digest, size, mediaType, annotations

**2. Rewrite Timestamps (optional)**

If `SOURCE_DATE_EPOCH` set:

```go
func (ic *ImageWriter) rewriteRemoteWithEpoch(
    ctx, opts, remote, baseImg,
) (*solver.Remote, error) {
    // For each layer:
    for _, desc := range remote.Descriptors {
        // Extract layer tarball
        // Rewrite file timestamps to epoch
        // Recompress
        // Update descriptor with new digest
    }
}
```

**3. Create Image Config**

```go
func commitDistributionManifest(
    ctx, opts, ref, config, remote, annotations, inlineCache, epoch, sessionGroup, baseImg,
) (*ocispecs.Descriptor, *ocispecs.Descriptor, error) {
    // Parse or create default config
    config, err := patchImageConfig(config, layers, history, inlineCache, epoch, baseImg)

    // Config includes:
    // - Architecture, OS
    // - RootFS diff IDs
    // - History entries
    // - Env, Cmd, Entrypoint, etc.
    configDigest := digest.FromBytes(config)
}
```

**4. Create Manifest**

```go
mfst := ocispecs.Manifest{
    MediaType: ocispecs.MediaTypeImageManifest,  // or Docker v2
    Config: ocispecs.Descriptor{
        MediaType: ocispecs.MediaTypeImageConfig,
        Digest:    configDigest,
        Size:      len(config),
    },
    Layers: []ocispecs.Descriptor{
        {
            MediaType: ocispecs.MediaTypeImageLayerGzip,
            Digest:    layerDigest,
            Size:      layerSize,
            Annotations: {
                "containerd.io/uncompressed": diffID,
            },
        },
        // ... more layers
    },
    Annotations: manifestAnnotations,
}
```

**5. Write to ContentStore**

```go
// Write config
configDesc, err := content.WriteBlob(ctx, cs, configDigest, config)

// Write manifest
manifestJSON, err := json.Marshal(mfst)
manifestDigest := digest.FromBytes(manifestJSON)
manifestDesc, err := content.WriteBlob(ctx, cs, manifestDigest, manifestJSON)

return &manifestDesc, &configDesc, nil
```

### Multi-Platform Images

**Location**: `exporter/containerimage/writer.go:187-351`

For multi-platform builds:

```go
// 1. Create manifest for each platform
var manifests []ocispecs.Descriptor
for _, platform := range platforms {
    ref := refs[platform.ID]
    mfstDesc, configDesc, err := ic.commitDistributionManifest(
        ctx, opts, ref, config, remote, annotations, inlineCache, epoch, sessionGroup, baseImg,
    )
    mfstDesc.Platform = &platform.Platform
    manifests = append(manifests, *mfstDesc)
}

// 2. Create manifest index (image list)
idx := ocispecs.Index{
    MediaType: ocispecs.MediaTypeImageIndex,
    Manifests: manifests,
    Annotations: indexAnnotations,
}

// 3. Write index
idxJSON, err := json.Marshal(idx)
idxDigest := digest.FromBytes(idxJSON)
idxDesc, err := content.WriteBlob(ctx, cs, idxDigest, idxJSON)

return &idxDesc, nil
```

**Structure**:
```
Index (sha256:abc...)
  ├─ Manifest linux/amd64 (sha256:def...)
  │   ├─ Config (sha256:123...)
  │   └─ Layers
  │       ├─ Layer 0 (sha256:456...)
  │       └─ Layer 1 (sha256:789...)
  ├─ Manifest linux/arm64 (sha256:ghi...)
  │   ├─ Config (sha256:234...)
  │   └─ Layers
  │       ├─ Layer 0 (sha256:567...)
  │       └─ Layer 1 (sha256:890...)
  └─ Attestations (optional)
      └─ Attestation Manifest (sha256:jkl...)
```

### Containerd Storage

**When store=true**:

```go
// 1. Store in containerd Images API
img := images.Image{
    Name:   "docker.io/myimage:latest",
    Target: manifestDesc,  // Points to manifest or index
}
_, err := imagesStore.Create(ctx, img)

// 2. Also create canonical name with digest
img.Name = "docker.io/myimage@sha256:abc123..."
_, err := imagesStore.Update(ctx, img)
```

**ContentStore layout**:
```
/var/lib/buildkit/containerd/content/
  └─ blobs/
      └─ sha256/
          ├─ abc123... (index or manifest)
          ├─ def456... (config)
          ├─ 789012... (layer 0)
          └─ 345678... (layer 1)
```

### Registry Push

**Location**: `exporter/containerimage/export.go:386-466`

**When push=true**:

```go
func (e *imageExporterInstance) pushImage(
    ctx, src, sessionID, targetName, manifestDigest,
) error {
    // 1. Collect all content providers
    mprovider := contentutil.NewMultiProvider(contentStore)
    for _, ref := range refs {
        remotes, err := ref.GetRemotes(ctx, false, refCfg, false, sessionGroup)
        for _, desc := range remote.Descriptors {
            mprovider.Add(desc.Digest, remote.Provider)
        }
    }

    // 2. Parse registry reference
    ref, err := reference.ParseNormalizedNamed(targetName)
    // e.g., "docker.io/myimage:latest"

    // 3. Get registry auth from session
    sessionGroup := session.NewGroup(sessionID)
    resolver := docker.NewResolver(docker.ResolverOptions{
        Hosts: registryHosts,
    })

    // 4. Push manifest and layers
    err := push.Push(ctx, sessionGroup, resolver, targetName, mprovider, manifestDigest)

    return nil
}
```

**Push process**:
1. Check if layers already exist (HEAD request)
2. Upload missing layers (chunked uploads for large blobs)
3. Upload config
4. Upload manifest
5. Create/update tag

### OCI Archive Export

**Location**: `exporter/oci/export.go:134-296`

**When tar=true**:

```go
func (e *imageExporterInstance) Export(
    ctx, src, buildInfo,
) (map[string]string, exporter.DescriptorReference, error) {
    // 1. Commit to descriptor
    desc, err := e.opt.ImageWriter.Commit(ctx, src, sessionID, inlineCache, &opts)

    // 2. Collect all content providers
    mprovider := contentutil.NewMultiProvider(contentStore)
    for _, ref := range refs {
        remotes, err := ref.GetRemotes(ctx, false, refCfg, false, sessionGroup)
        for _, desc := range remote.Descriptors {
            mprovider.Add(desc.Digest, remote.Provider)
        }
    }

    // 3. Export as tar via session
    w, err := filesync.CopyFileWriter(ctx, resp, e.id, caller)

    expOpts := []archiveexporter.ExportOpt{
        archiveexporter.WithManifest(*desc, names...),
        archiveexporter.WithAllPlatforms(),         // OCI variant
        archiveexporter.WithSkipDockerManifest(),   // OCI variant
    }

    err := archiveexporter.Export(ctx, mprovider, w, expOpts...)

    return resp, nil, w.Close()
}
```

**OCI Archive structure** (tar contents):

```
oci-layout                    # {"imageLayoutVersion": "1.0.0"}
index.json                    # Image index
blobs/
  └─ sha256/
      ├─ abc123...            # Manifest or Index
      ├─ def456...            # Config
      ├─ 789012...            # Layer 0 (compressed)
      └─ 345678...            # Layer 1 (compressed)
```

**index.json** example:
```json
{
  "schemaVersion": 2,
  "manifests": [
    {
      "mediaType": "application/vnd.oci.image.manifest.v1+json",
      "digest": "sha256:abc123...",
      "size": 1234,
      "annotations": {
        "org.opencontainers.image.ref.name": "latest"
      }
    }
  ]
}
```

### Docker Archive Export

**When type="docker", tar=true**:

The Docker archive format is specifically designed for compatibility with `docker load` and `docker save`. This is different from the raw filesystem tar and OCI tar formats.

**Location**: `exporter/containerimage/export.go`, `util/imageutil/archive.go`

**Exporter Configuration for docker load**:

```go
Exporter {
    Type: "docker",  // Use "docker" exporter, NOT "tar"
    Attrs: map[string]string{
        "tar": "true",              // Export as tar archive
        "name": "myapp:latest",     // Required: Image name and tag
        // Optional attributes:
        "compression": "gzip",      // Layer compression
        "oci-mediatypes": "false",  // Use Docker types (default)
    }
}
```

**Key Requirements for docker load compatibility**:
1. **Must use `"docker"` exporter type** - NOT `"tar"` or `"oci"`
2. **Must specify `name` attribute** - Required for tag mapping
3. **Must set `tar=true`** - To get archive instead of loading to containerd
4. **Single platform only** - Multi-platform not supported by docker load
5. **Uses Docker v2 Schema 2 format** - Not OCI format

**Docker Archive structure**:
```
manifest.json                 # Docker v2 manifest list (critical for docker load)
<config-digest>.json          # Image config (referenced by manifest)
<layer-digest>/               # Layer directories
  ├─ layer.tar                # Actual layer content
  ├─ json                     # Layer metadata
  └─ VERSION                  # Layer version file
repositories                  # Tag to manifest mapping (legacy, optional)
```

**manifest.json format** (critical for `docker load`):

```json
[
  {
    "Config": "sha256abc123.json",
    "RepoTags": ["myapp:latest"],
    "Layers": [
      "sha256def456/layer.tar",
      "sha256ghi789/layer.tar"
    ]
  }
]
```

**Implementation Details**:

```go
func exportDockerArchive(
    ctx context.Context,
    mprovider content.Provider,
    manifestDesc ocispecs.Descriptor,
    names []string,
) error {
    // 1. Load manifest
    manifest, err := readManifest(ctx, mprovider, manifestDesc)

    // 2. Create tar writer
    tw := tar.NewWriter(output)

    // 3. Write config
    configPath := manifest.Config.Digest.Encoded() + ".json"
    configData, _ := content.ReadBlob(ctx, mprovider, manifest.Config)
    tw.WriteHeader(&tar.Header{
        Name: configPath,
        Size: len(configData),
        Mode: 0644,
    })
    tw.Write(configData)

    // 4. Write layers
    var layerPaths []string
    for _, layerDesc := range manifest.Layers {
        layerDir := layerDesc.Digest.Encoded()
        layerPath := layerDir + "/layer.tar"
        layerPaths = append(layerPaths, layerPath)

        // Extract layer content
        layerData, _ := content.ReadBlob(ctx, mprovider, layerDesc)

        // Write layer.tar
        tw.WriteHeader(&tar.Header{
            Name: layerPath,
            Size: len(layerData),
            Mode: 0644,
        })
        tw.Write(layerData)

        // Write VERSION file
        tw.WriteHeader(&tar.Header{
            Name: layerDir + "/VERSION",
            Size: 3,
            Mode: 0644,
        })
        tw.Write([]byte("1.0"))

        // Write json metadata
        layerConfig := createLayerConfig(layerDesc)
        layerConfigJSON, _ := json.Marshal(layerConfig)
        tw.WriteHeader(&tar.Header{
            Name: layerDir + "/json",
            Size: len(layerConfigJSON),
            Mode: 0644,
        })
        tw.Write(layerConfigJSON)
    }

    // 5. Write manifest.json (CRITICAL for docker load)
    manifestJSON := []dockerManifest{
        {
            Config:   configPath,
            RepoTags: names,  // ["myapp:latest"]
            Layers:   layerPaths,
        },
    }
    manifestData, _ := json.Marshal(manifestJSON)
    tw.WriteHeader(&tar.Header{
        Name: "manifest.json",
        Size: len(manifestData),
        Mode: 0644,
    })
    tw.Write(manifestData)

    // 6. Write repositories (legacy compatibility)
    repositories := map[string]map[string]string{}
    for _, name := range names {
        repo, tag := parseRepoTag(name)
        if repositories[repo] == nil {
            repositories[repo] = make(map[string]string)
        }
        repositories[repo][tag] = manifest.Layers[len(manifest.Layers)-1].Digest.Encoded()
    }
    repoData, _ := json.Marshal(repositories)
    tw.WriteHeader(&tar.Header{
        Name: "repositories",
        Size: len(repoData),
        Mode: 0644,
    })
    tw.Write(repoData)

    return tw.Close()
}
```

**Differences from OCI Archive**:

| Feature | Docker Archive | OCI Archive |
|---------|----------------|-------------|
| Entry point | `manifest.json` | `index.json` with `oci-layout` |
| Layer storage | `<digest>/layer.tar` | `blobs/sha256/<digest>` |
| Compatibility | `docker load` | OCI runtimes, `skopeo` |
| Multi-platform | No | Yes |
| Media types | Docker v2 | OCI |
| Additional files | `VERSION`, `json` per layer | None |

**Practical Usage with buildctl**:

```bash
# Export Docker archive compatible with docker load
buildctl build \
    --frontend dockerfile.v0 \
    --local context=/path/to/context \
    --local dockerfile=/path/to/dockerfile \
    --output type=docker,name=myapp:v1.0,dest=myapp.tar

# Load into Docker
docker load < myapp.tar
# Output: Loaded image: myapp:v1.0

# Verify
docker images | grep myapp
```

**Common Issues and Solutions**:

1. **Error: "no manifest.json found"**
   - Cause: Using `type=tar` or `type=oci` instead of `type=docker`
   - Solution: Use `type=docker,tar=true`

2. **Error: "invalid tar header"**
   - Cause: Missing `name` attribute
   - Solution: Always specify `name=imagename:tag`

3. **Error: "image not found after load"**
   - Cause: `manifest.json` has empty or incorrect `RepoTags`
   - Solution: Ensure `name` attribute is set correctly

4. **Multi-platform build fails**
   - Cause: Docker archive doesn't support multi-platform
   - Solution: Use `--platform` to select single platform, or use OCI export

**Stream-based Export Flow**:

```
Server                                          Client
------                                          ------
[Build complete with cache.ImmutableRef]
    |
    v
exporter/containerimage:
  commitDistributionManifest()
    |-- Create config JSON
    |-- Create manifest with layer descriptors
    |-- Write to content store
    |
    v
exportDockerArchive():
  Walk content store
  Build tar structure:
    1. Config file
    2. Layer directories with:
       - layer.tar (actual data)
       - json (metadata)
       - VERSION
    3. manifest.json (critical)
    4. repositories (legacy)
    |
    v
  CopyFileWriter(ctx, exporterID, caller)
    |-- FileSend.DiffCopy(ctx) -----------------> [FileSend server]
    |                                                |
    |                                                v
    |                                            wc = os.Create("myapp.tar")
    |                                                |
Write tar chunks:                                    |
    |-- BytesMessage{data: [3MB chunk]} ----------->|-- wc.Write(chunk)
    |-- BytesMessage{data: [3MB chunk]} ----------->|-- wc.Write(chunk)
    |-- BytesMessage{data: [remaining]} ----------->|-- wc.Write(chunk)
    |                                                |
Close stream                                         |
    |-- CloseSend() ------------------------------>|
    |                                                |
    |                                                v
    |                                            wc.Close()
    |<-- EOF ----------------------------------------|
    |                                                |
    v                                                v
Complete                                    myapp.tar ready
                                            (docker load compatible)
```

**Verification of Docker Archive**:

```bash
# Extract and inspect
mkdir inspect
cd inspect
tar xf ../myapp.tar

# Check structure
ls -la
# Should show:
# - manifest.json
# - repositories
# - <digest>.json (config)
# - <digest>/ directories (layers)

# Inspect manifest
cat manifest.json | jq
# Should show Config, RepoTags, Layers

# Check layer
ls <digest>/
# Should show: layer.tar, json, VERSION
```

### Exporter Options

**Common options**:

```go
attrs := map[string]string{
    // Naming
    "name": "docker.io/myimage:latest",
    "name-canonical": "true",  // Add @digest variant

    // Storage
    "push": "true",            // Push to registry
    "store": "true",           // Store in containerd
    "unpack": "true",          // Unpack for runtime

    // Format
    "oci-mediatypes": "true",  // Use OCI types vs Docker
    "compression": "zstd",     // gzip, zstd, uncompressed
    "compression-level": "3",
    "force-compression": "true",

    // Timestamps
    "rewrite-timestamp": "true",  // Requires SOURCE_DATE_EPOCH

    // Annotations
    "annotation[org.opencontainers.image.source]": "https://github.com/...",
    "annotation-manifest[key]": "value",
    "annotation-index[key]": "value",
}
```

### Attestations Support

**Location**: `exporter/containerimage/writer.go:188-350`

BuildKit can attach attestations (SBOM, provenance) to images:

```go
// For each platform with attestations:
if len(platformAttestations) > 0 {
    // Create attestation manifest
    attManifest := ocispecs.Manifest{
        MediaType: attestationManifestArtifactType,
        Subject:   platformManifestDesc,  // Points to image manifest
        Layers: []ocispecs.Descriptor{
            // Attestation documents (JSON)
        },
    }

    // Add to index
    idx.Manifests = append(idx.Manifests, attManifestDesc)
}
```

**Attestation types**:
- SLSA Provenance (v0.2, v1.0)
- SPDX SBOM
- CycloneDX SBOM
- Custom attestations

### Content Addressing

All image components are content-addressed:

```go
// Layers identified by:
layerDigest := sha256(compressedLayerTar)
diffID := sha256(uncompressedLayerTar)

// Config identified by:
configDigest := sha256(configJSON)

// Manifest identified by:
manifestDigest := sha256(manifestJSON)

// Index identified by:
indexDigest := sha256(indexJSON)
```

**Chain IDs** for cache:
```go
chainID[0] = diffID[0]
chainID[1] = sha256(chainID[0] + " " + diffID[1])
chainID[n] = sha256(chainID[n-1] + " " + diffID[n])
```

### Complete Image Export Example

**Request**:
```go
solveReq := &controlapi.SolveRequest{
    Ref:        "build-123",
    Definition: llbDefinition,
    Session:    session.ID(),
    Exporters: []*controlapi.Exporter{
        {
            Type: "image",
            Attrs: map[string]string{
                "name":             "docker.io/myapp:v1.0",
                "push":             "true",
                "store":            "true",
                "oci-mediatypes":   "true",
                "compression":      "zstd",
            },
        },
    },
}
```

**Server execution**:
```
1. Solve LLB → cache.ImmutableRef

2. ImageWriter.Commit():
   a. exportLayers() → compress, create descriptors
   b. rewriteRemoteWithEpoch() → if SOURCE_DATE_EPOCH
   c. commitDistributionManifest():
      - Create image config
      - Create manifest
      - Write to content store

3. Store in containerd (if store=true):
   - images.Create("myapp:v1.0", manifestDesc)
   - images.Create("myapp@sha256:...", manifestDesc)

4. Push to registry (if push=true):
   - Resolve docker.io/myapp:v1.0
   - Upload layers (if not exists)
   - Upload config
   - Upload manifest
   - Tag v1.0 → manifestDigest

5. Return response:
   {
     "containerimage.digest": "sha256:abc123...",
     "containerimage.config.digest": "sha256:def456...",
     "containerimage.descriptor": "<base64-encoded descriptor>"
   }
```

**Client receives**:
- Image digest for referencing
- Config digest for verification
- Full descriptor for inspection

### Lazy Pulling

BuildKit supports lazy pulling with stargz/eStargz:

```go
if unlazier, ok := remote.Provider.(cache.Unlazier); ok {
    // Ensure all blobs are present before push/store
    err := unlazier.Unlazy(ctx)
}
```

Layers can remain remote until actually needed.

### Inline Cache

**When inline-cache=true**:

```go
// Embed cache metadata in image config
config["moby.buildkit.cache.v0"] = {
    "type": "layers",
    "layers": [
        {
            "blob": "sha256:...",
            "diffID": "sha256:...",
            "cacheID": "...",
        },
    ],
}
```

Allows using the image itself as cache source.

---

## Practical Guide: Creating docker load Compatible Tar Archives

This section provides comprehensive examples for creating tar archives that can be loaded with `docker load` or used with OCI tooling.

### Export Types Comparison

BuildKit provides three different exporters that create tar archives, each with different purposes:

| Exporter | Type | Output | Compatible With | Use Case |
|----------|------|--------|-----------------|----------|
| **Tar** | `tar` | Raw filesystem tar | `tar xf`, filesystem operations | Extract rootfs, debug |
| **Docker** | `docker` | Docker v2 image tar | `docker load` | Docker distribution |
| **OCI** | `oci` | OCI image layout tar | `skopeo copy`, OCI runtimes | OCI-compliant distribution |

### Method 1: Docker Archive Export (Recommended for docker load)

**Using buildctl CLI**:

```bash
# Basic export
buildctl build \
    --frontend dockerfile.v0 \
    --local context=. \
    --local dockerfile=. \
    --output type=docker,name=myapp:v1.0,dest=myapp.tar

# With compression options
buildctl build \
    --frontend dockerfile.v0 \
    --local context=. \
    --local dockerfile=. \
    --output type=docker,name=myapp:latest,dest=myapp.tar,compression=gzip,compression-level=9

# Multiple tags
buildctl build \
    --frontend dockerfile.v0 \
    --local context=. \
    --output "type=docker,\"name=myapp:v1.0,myapp:latest\",dest=myapp.tar"
```

**Using Go client API**:

```go
package main

import (
    "context"
    "os"

    "github.com/moby/buildkit/client"
    "github.com/moby/buildkit/client/llb"
    "github.com/moby/buildkit/session"
    "github.com/moby/buildkit/session/auth/authprovider"
    "github.com/moby/buildkit/session/filesync"
    controlapi "github.com/moby/buildkit/api/services/control"
)

func exportDockerTar(ctx context.Context, c *client.Client, llbDef *llb.Definition) error {
    // Create session
    sess, err := session.NewSession(ctx, "")
    if err != nil {
        return err
    }

    // Auth provider for pulling base images
    sess.Allow(authprovider.NewDockerAuthProvider(
        authprovider.DockerAuthProviderConfig{}))

    // File sync target for receiving tar
    sess.Allow(filesync.NewFSSyncTarget(
        filesync.WithFSSync(0, func(md map[string]string) (io.WriteCloser, error) {
            return os.Create("output.tar")
        })))

    // Start session
    go sess.Run(ctx, grpchijack.Dialer(c.ControlClient()))

    // Marshal LLB definition
    def, err := llbDef.Marshal(ctx)
    if err != nil {
        return err
    }

    // Solve request with docker exporter
    solveReq := &controlapi.SolveRequest{
        Ref:        "build-docker-tar",
        Definition: def.ToPB(),
        Session:    sess.ID(),
        Exporters: []*controlapi.Exporter{
            {
                Type: "docker",  // IMPORTANT: Use "docker" not "tar"
                Attrs: map[string]string{
                    "name":        "myapp:v1.0",    // Required
                    "tar":         "true",          // Export as tar file
                    "compression": "gzip",
                },
            },
        },
    }

    resp, err := c.ControlClient().Solve(ctx, solveReq)
    if err != nil {
        return err
    }

    fmt.Printf("Export complete. Image digest: %s\n",
        resp.ExporterResponse["containerimage.digest"])
    return nil
}
```

**Direct gRPC usage**:

```go
import (
    controlapi "github.com/moby/buildkit/api/services/control"
    "google.golang.org/grpc"
)

func exportViaGRPC(ctx context.Context, conn *grpc.ClientConn, llbDef []byte, sessionID string) error {
    client := controlapi.NewControlClient(conn)

    req := &controlapi.SolveRequest{
        Ref:        "my-build-ref",
        Definition: &pb.Definition{Def: llbDef},
        Session:    sessionID,
        Exporters: []*controlapi.Exporter{
            {
                Type: "docker",
                Attrs: map[string]string{
                    "name": "myimage:tag",
                    "tar":  "true",
                },
            },
        },
    }

    resp, err := client.Solve(ctx, req)
    return err
}
```

### Method 2: OCI Archive Export (For OCI Tooling)

**Using buildctl CLI**:

```bash
# Export OCI image layout tar
buildctl build \
    --frontend dockerfile.v0 \
    --local context=. \
    --local dockerfile=. \
    --output type=oci,dest=myapp-oci.tar

# With name annotation
buildctl build \
    --frontend dockerfile.v0 \
    --local context=. \
    --output type=oci,name=myapp:v1.0,dest=myapp-oci.tar

# Load with skopeo (OCI tooling)
skopeo copy oci-archive:myapp-oci.tar docker-daemon:myapp:v1.0

# Or import to docker using skopeo
skopeo copy oci-archive:myapp-oci.tar docker://localhost:5000/myapp:v1.0
```

**Using Go client API**:

```go
func exportOCITar(ctx context.Context, c *client.Client, llbDef *llb.Definition) error {
    sess, _ := setupSession(ctx)
    go sess.Run(ctx, grpchijack.Dialer(c.ControlClient()))

    def, _ := llbDef.Marshal(ctx)

    solveReq := &controlapi.SolveRequest{
        Ref:        "build-oci-tar",
        Definition: def.ToPB(),
        Session:    sess.ID(),
        Exporters: []*controlapi.Exporter{
            {
                Type: "oci",
                Attrs: map[string]string{
                    "name":            "myapp:v1.0",
                    "tar":             "true",
                    "compression":     "zstd",
                    "oci-mediatypes":  "true",
                },
            },
        },
    }

    _, err := c.ControlClient().Solve(ctx, solveReq)
    return err
}
```

**OCI Archive contents**:

```bash
# Extract and examine
tar xf myapp-oci.tar
ls -la

# Output:
# oci-layout         - {"imageLayoutVersion": "1.0.0"}
# index.json         - Image index
# blobs/
#   sha256/
#     abc123...      - Manifest
#     def456...      - Config
#     789012...      - Layer 0
#     345678...      - Layer 1
```

### Method 3: Multi-Format Export (Multiple Exporters)

You can specify multiple exporters in a single build:

```bash
# Export both Docker tar and push to registry
buildctl build \
    --frontend dockerfile.v0 \
    --local context=. \
    --local dockerfile=. \
    --output type=docker,name=myapp:v1.0,dest=myapp.tar \
    --output type=image,name=localhost:5000/myapp:v1.0,push=true
```

**Using Go client with multiple exporters**:

```go
solveReq := &controlapi.SolveRequest{
    Ref:        "multi-export-build",
    Definition: def.ToPB(),
    Session:    sess.ID(),
    Exporters: []*controlapi.Exporter{
        // Export 1: Docker tar for local use
        {
            Type: "docker",
            Attrs: map[string]string{
                "name": "myapp:v1.0",
                "tar":  "true",
            },
        },
        // Export 2: Push to registry
        {
            Type: "image",
            Attrs: map[string]string{
                "name": "registry.example.com/myapp:v1.0",
                "push": "true",
            },
        },
        // Export 3: Store in containerd
        {
            Type: "image",
            Attrs: map[string]string{
                "name":  "myapp:latest",
                "store": "true",
            },
        },
    },
}
```

### Complete Example: LLB to Docker Tar

```go
package main

import (
    "context"
    "fmt"
    "io"
    "net"
    "os"

    "github.com/moby/buildkit/client"
    "github.com/moby/buildkit/client/llb"
    "github.com/moby/buildkit/session"
    "github.com/moby/buildkit/session/auth/authprovider"
    "github.com/moby/buildkit/session/filesync"
    "github.com/moby/buildkit/session/grpchijack"
    controlapi "github.com/moby/buildkit/api/services/control"
    "google.golang.org/grpc"
    "google.golang.org/grpc/credentials/insecure"
)

func main() {
    ctx := context.Background()

    // 1. Connect to buildkitd
    conn, err := grpc.Dial("unix:///run/buildkit/buildkitd.sock",
        grpc.WithTransportCredentials(insecure.NewCredentials()))
    if err != nil {
        panic(err)
    }
    defer conn.Close()

    c, err := client.New(ctx, "", client.WithContextDialer(
        func(context.Context, string) (net.Conn, error) {
            return grpc.DialContext(ctx, "unix:///run/buildkit/buildkitd.sock",
                grpc.WithTransportCredentials(insecure.NewCredentials()))
        }))
    if err != nil {
        panic(err)
    }

    // 2. Create LLB definition
    // Example: Build from Alpine, install curl
    state := llb.Image("docker.io/library/alpine:3.18")
    state = state.Run(
        llb.Shlex("apk add --no-cache curl"),
    ).Root()

    // Add metadata to final image
    state = state.Run(
        llb.Shlex("echo 'Built with BuildKit' > /etc/motd"),
    ).Root()

    def, err := state.Marshal(ctx)
    if err != nil {
        panic(err)
    }

    // 3. Setup session
    sess, err := session.NewSession(ctx, "docker-export-example")
    if err != nil {
        panic(err)
    }

    // Auth provider for pulling images
    sess.Allow(authprovider.NewDockerAuthProvider(
        authprovider.DockerAuthProviderConfig{},
    ))

    // File sync target for receiving tar
    outputFile := "myapp.tar"
    sess.Allow(filesync.NewFSSyncTarget(
        filesync.WithFSSync(0, func(md map[string]string) (io.WriteCloser, error) {
            fmt.Printf("Receiving Docker tar archive...\n")
            return os.Create(outputFile)
        }),
    ))

    // Start session in background
    go func() {
        err := sess.Run(ctx, grpchijack.Dialer(c.ControlClient()))
        if err != nil {
            fmt.Printf("Session error: %v\n", err)
        }
    }()

    // 4. Create solve request with docker exporter
    solveReq := &controlapi.SolveRequest{
        Ref:        "alpine-with-curl",
        Definition: def.ToPB(),
        Session:    sess.ID(),
        Exporters: []*controlapi.Exporter{
            {
                Type: "docker",
                Attrs: map[string]string{
                    "name":                 "myapp:v1.0",
                    "tar":                  "true",
                    "compression":          "gzip",
                    "compression-level":    "6",
                    "force-compression":    "true",
                    // Optional: reproducible builds
                    "rewrite-timestamp":    "true",
                    // Set via frontend attrs: SOURCE_DATE_EPOCH
                },
            },
        },
        FrontendAttrs: map[string]string{
            "source-date-epoch": "1234567890",  // Optional: for reproducibility
        },
    }

    // 5. Execute build
    fmt.Println("Starting build...")
    resp, err := c.ControlClient().Solve(ctx, solveReq)
    if err != nil {
        panic(err)
    }

    // 6. Print results
    fmt.Printf("\nBuild complete!\n")
    fmt.Printf("Docker archive: %s\n", outputFile)
    if digest, ok := resp.ExporterResponse["containerimage.digest"]; ok {
        fmt.Printf("Image digest: %s\n", digest)
    }
    if configDigest, ok := resp.ExporterResponse["containerimage.config.digest"]; ok {
        fmt.Printf("Config digest: %s\n", configDigest)
    }

    // 7. Verify and load
    fmt.Println("\nTo load into Docker:")
    fmt.Printf("  docker load < %s\n", outputFile)
    fmt.Println("\nTo verify archive structure:")
    fmt.Printf("  tar tf %s | head -20\n", outputFile)
}
```

### Advanced: Platform-Specific Exports

For multi-platform builds, you need to export each platform separately for `docker load`:

```bash
# Build for linux/amd64
buildctl build \
    --frontend dockerfile.v0 \
    --opt platform=linux/amd64 \
    --local context=. \
    --local dockerfile=. \
    --output type=docker,name=myapp:v1.0-amd64,dest=myapp-amd64.tar

# Build for linux/arm64
buildctl build \
    --frontend dockerfile.v0 \
    --opt platform=linux/arm64 \
    --local context=. \
    --local dockerfile=. \
    --output type=docker,name=myapp:v1.0-arm64,dest=myapp-arm64.tar

# Load the appropriate one
docker load < myapp-$(uname -m).tar
```

**For multi-platform with OCI** (single tar, all platforms):

```bash
# Build multi-platform OCI archive
buildctl build \
    --frontend dockerfile.v0 \
    --opt platform=linux/amd64,linux/arm64 \
    --local context=. \
    --local dockerfile=. \
    --output type=oci,dest=myapp-multiplatform.tar

# Import with skopeo (preserves all platforms)
skopeo copy oci-archive:myapp-multiplatform.tar docker://localhost:5000/myapp:v1.0
```

### Exporter Attributes Reference

#### Docker Exporter Attributes

```go
map[string]string{
    // Required
    "name":                      "image:tag",           // Image name and tag
    "tar":                       "true",                // Export as tar file

    // Compression
    "compression":               "gzip|zstd|uncompressed",
    "compression-level":         "0-9",                 // Compression level
    "force-compression":         "true|false",          // Force recompression

    // Timestamps (reproducible builds)
    "rewrite-timestamp":         "true|false",          // Rewrite timestamps
    // Requires SOURCE_DATE_EPOCH in FrontendAttrs

    // Media types
    "oci-mediatypes":            "false",               // Use Docker types (default)

    // Annotations (limited support in docker format)
    "annotation[key]":           "value",
}
```

#### OCI Exporter Attributes

```go
map[string]string{
    // Optional
    "name":                      "image:tag",           // Image reference
    "tar":                       "true",                // Export as tar file

    // Compression
    "compression":               "gzip|zstd|uncompressed",
    "compression-level":         "0-9",
    "force-compression":         "true|false",

    // Format
    "oci-mediatypes":            "true",                // Use OCI types (default)

    // Annotations (full OCI support)
    "annotation[org.opencontainers.image.source]": "https://github.com/...",
    "annotation-manifest[key]":  "value",               // Manifest annotations
    "annotation-index[key]":     "value",               // Index annotations

    // Timestamps
    "rewrite-timestamp":         "true|false",
}
```

### Troubleshooting Common Issues

#### Issue: "cannot export to docker, image must have exactly one platform"

**Cause**: Building for multiple platforms with docker exporter

**Solution**:
```bash
# Option 1: Select single platform
buildctl build --opt platform=linux/amd64 \
    --output type=docker,name=myapp:v1.0,dest=myapp.tar

# Option 2: Use OCI exporter instead
buildctl build --opt platform=linux/amd64,linux/arm64 \
    --output type=oci,dest=myapp.tar
```

#### Issue: "name is required for docker exporter"

**Cause**: Missing `name` attribute

**Solution**:
```go
Attrs: map[string]string{
    "name": "myimage:tag",  // Add this
    "tar":  "true",
}
```

#### Issue: "docker load" returns "no such file or directory"

**Cause**: Using tar or oci exporter instead of docker

**Solution**:
```bash
# Wrong: type=tar (raw filesystem)
buildctl build --output type=tar,dest=out.tar

# Wrong: type=oci (OCI layout, needs skopeo)
buildctl build --output type=oci,dest=out.tar

# Correct: type=docker
buildctl build --output type=docker,name=myapp:v1.0,dest=out.tar
```

#### Issue: Archive is too large

**Solution**: Enable compression
```bash
buildctl build \
    --output type=docker,name=myapp:v1.0,dest=myapp.tar,\
compression=zstd,compression-level=9,force-compression=true
```

### Performance Considerations

**Compression trade-offs**:

| Method | Speed | Ratio | CPU | Best For |
|--------|-------|-------|-----|----------|
| uncompressed | Fastest | 1.0x | Low | Local development |
| gzip,level=1 | Fast | 2-3x | Medium | Quick exports |
| gzip,level=6 | Medium | 3-4x | Medium | Balanced (default) |
| gzip,level=9 | Slow | 4-5x | High | Size-critical |
| zstd,level=3 | Fast | 3-4x | Medium | Modern default |
| zstd,level=19 | Very slow | 5-6x | Very high | Maximum compression |

**Recommendation**:
- Development: `compression=gzip,compression-level=1` or `uncompressed`
- Production: `compression=zstd,compression-level=3`
- Size-critical: `compression=zstd,compression-level=9`

### Summary: Quick Reference

**For docker load** (most common):
```bash
buildctl build \
    --output type=docker,name=IMAGE:TAG,dest=FILE.tar
```

**For OCI tooling** (skopeo, etc.):
```bash
buildctl build \
    --output type=oci,name=IMAGE:TAG,dest=FILE.tar
```

**For filesystem extraction** (debugging):
```bash
buildctl build \
    --output type=tar,dest=FILE.tar
```

**For direct registry push** (no tar file):
```bash
buildctl build \
    --output type=image,name=REGISTRY/IMAGE:TAG,push=true
```

---

## Conclusion

The BuildKit gRPC communication pattern follows a well-defined sequence:

1. **Session establishment** for multiplexed communication
2. **Solve request** with LLB definition and exporter config
3. **Parallel execution** of LLB DAG by solver
4. **Export** via specified exporter (tar uses session file sync)
5. **Status streaming** for real-time progress
6. **Completion** with exporter response and metadata

This architecture enables:
- **Efficient caching** through content-addressable storage
- **Parallel execution** of independent operations
- **Flexible export** to multiple formats simultaneously
- **Secure builds** with entitlements and secret management
- **Real-time feedback** via status streaming

### Key Takeaways

**FileSync Protocol**:
- Bidirectional file transfer over session stream
- Client implements both FileSync and FileSend services
- Supports chunked transfer (3MB chunks) for large files
- Includes metadata filtering and content hashing

**Local Context**:
- Client registers local directories with session
- Server requests files via FileSync.DiffCopy RPC
- Files transferred as Packet stream with metadata
- Cached using SharedKey for incremental updates

**Tar Export**:
- Server streams tar to client via FileSend.DiffCopy
- Client provides WriteCloser callback
- Transfer uses BytesMessage chunks (3MB max)
- Blocking flow ensures complete transfer

**Client Requirements**:
- Must implement session.Attachable for services
- Must provide DirSource for local directories
- Must provide FileOutputFunc for exports
- Should handle auth, secrets, SSH as needed

**Export Mechanisms**:
- **Tar export**: Streams raw filesystem tar via FileSend.DiffCopy
- **OCI export**: Creates OCI Image Layout tar with manifest/config/layers
- **Image export**: Stores in containerd and/or pushes to registry
- All exports use content-addressable storage for deduplication
- Multi-platform images supported via manifest index
- Attestations (SBOM, provenance) can be attached to images

The tar export leverages the session file sync protocol to stream the tar archive directly to the client without intermediate storage. OCI and Docker image exports create proper image manifests with config and layers, optionally pushing to registries or storing in the local containerd image store, making them suitable for both distribution and runtime use.
