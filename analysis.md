# BuildKit Implementation Bug Analysis

## Summary
Compared current implementation against `/tmp/buildkit/BUILDKIT_GRPC_LLB_TAR_EXPORT_FLOW.md` (comprehensive BuildKit gRPC protocol documentation). Found **2 critical bugs** and **3 minor issues**.

## Critical Bugs (Blocking Tar Export)

### 🔴 BUG #1: Missing FileSend Service Implementation
**Status:** CRITICAL - Tar export will fail completely
**File:** `src/buildkit/session.rs:178-181`

**Problem:**
According to BuildKit documentation (lines 820-870, "Two Directions of File Transfer"):
- **FileSync service** (Client → Server): For uploading build context ✅ Implemented
- **FileSend service** (Server → Client): For downloading tar exports ❌ **NOT Implemented**

When BuildKit exports to tar, it:
1. Creates a FileSend client pointing to the session
2. Calls `FileSend.DiffCopy(stream BytesMessage)` on the **client's session**
3. Streams tar as BytesMessage chunks (3MB max) to client
4. Client must implement FileSend server to receive the tar

**Current Code:**
```rust
// session.rs:178-181
let server = Server::builder()
    .add_service(FileSyncServerBuilder::new(filesync_service))  // ✅ For context upload
    .add_service(AuthServerBuilder::new(auth_service))         // ✅ For auth
    .serve_with_incoming(conn_stream);
// MISSING: FileSendServerBuilder for tar download ❌
```

**Impact:** When BuildKit tries to export tar, it will call FileSend.DiffCopy on the session, but the service isn't registered. gRPC will return "Unimplemented" error and tar export will fail.

**Reference:** BuildKit doc lines 1143-1362 ("Tar Export Return Path"), lines 473-485 (Session Protocol diagram)

---

### 🔴 BUG #2: Missing FileSend Proto Exports
**Status:** CRITICAL - Prevents implementing Bug #1 fix
**File:** `src/buildkit/proto.rs:46-48`

**Problem:**
Proto module only exports FileSync server but not FileSend server.

**Current Code:**
```rust
// proto.rs:46-48
pub use moby::filesync::v1::file_sync_server::{
    FileSync as FileSyncServer,
    FileSyncServer as FileSyncServerBuilder
};
// Missing FileSend server exports ❌
```

**Required Addition:**
```rust
pub use moby::filesync::v1::file_send_server::{
    FileSend as FileSendServer,
    FileSendServer as FileSendServerBuilder
};
```

**Impact:** Cannot implement FileSend service without these exports. Blocks Bug #1 fix.

**Reference:** BuildKit proto file `proto/filesync.proto:16-18` defines FileSend service

---

## Minor Issues (Non-Breaking)

### 🟡 ISSUE #3: Missing FileSend Method Advertisement
**Status:** Minor - May cause BuildKit to not route FileSend calls properly
**File:** `src/buildkit/session.rs:122-146`

**Problem:**
Session advertises available methods via `x-docker-expose-session-grpc-method` headers. Currently advertises:
- `/moby.filesync.v1.FileSync/DiffCopy` ✅
- `/moby.filesync.v1.FileSync/TarStream` ✅
- `/moby.filesync.v1.Auth/*` methods ✅
- `/moby.filesync.v1.FileSend/DiffCopy` ❌ Missing

**Required Addition:**
```rust
request.metadata_mut().append(
    "x-docker-expose-session-grpc-method",
    "/moby.filesync.v1.FileSend/DiffCopy".parse().context("Failed to parse FileSend method")?
);
```

**Impact:** BuildKit daemon may not know the session supports FileSend, though it might discover it dynamically. Best practice is to advertise all supported methods.

**Reference:** BuildKit doc lines 120-146 (Session Management), lines 461-472 (Session Attachables)

---

### 🟡 ISSUE #4: Incorrect Tar Exporter Attributes
**Status:** Minor - Sends unused data, doesn't break functionality
**File:** `src/buildkit/session.rs:271-279`

**Problem:**
Tar exporter configuration includes "name" attribute, which is used by **image exporters**, not tar exporters.

**Current Code:**
```rust
exporters: vec![
    super::proto::moby::buildkit::v1::Exporter {
        r#type: "tar".to_string(),
        attrs: [
            ("name".to_string(), image_tag.to_string()),  // ❌ Wrong attr
        ]
        .into_iter()
        .collect(),
    },
],
```

**Correct Configuration:**
According to doc lines 516-521, tar exporter attrs:
- `"epoch"` (optional): Set file timestamps to this Unix timestamp
- Empty map is valid for default behavior

**Fix:**
```rust
exporters: vec![
    super::proto::moby::buildkit::v1::Exporter {
        r#type: "tar".to_string(),
        attrs: Default::default(),  // Empty attrs for tar
    },
],
```

**Impact:** BuildKit ignores unknown attributes, so this doesn't break anything. Just sends unnecessary data.

**Reference:** BuildKit doc lines 89-99 (Exporter Configuration), lines 516-521 (Tar Export Specifics)

---

### 🟡 ISSUE #5: Missing Tar Output Handling
**Status:** Minor - Export request sent but tar bytes not saved
**File:** `src/buildkit/session.rs:218-318`

**Problem:**
Build request is sent with tar exporter, but there's no FileSend service to receive the tar bytes. Even after fixing Bugs #1-2, need to handle the received tar:
- Write to file (e.g., `/tmp/output.tar`)
- Or pipe to docker load
- Or return as bytes to caller

**Current Code:**
Only extracts metadata from response:
```rust
let image_id = solve_response
    .exporter_response
    .get("containerimage.digest")
    .cloned()
    .unwrap_or_else(|| format!("sha256:{}", self.session_id));
```

**Required:**
Implement FileSendService similar to FileSyncService:
- Receives BytesMessage stream from BuildKit
- Assembles chunks into complete tar
- Writes to output destination

**Reference:** BuildKit doc lines 1245-1362 (Complete Tar Export Flow)

---

## Implementation Plan

### Step 1: Add FileSend Proto Exports (Fixes Bug #2)
**File:** `src/buildkit/proto.rs`
```rust
// After line 48, add:
pub use moby::filesync::v1::file_send_server::{
    FileSend as FileSendServer,
    FileSendServer as FileSendServerBuilder
};
```

### Step 2: Implement FileSendService (Fixes Bug #1)
**File:** Create `src/buildkit/filesend_service.rs`

Similar to FileSyncService but simpler:
- Implements `FileSend` trait
- Has single method: `diff_copy(stream BytesMessage) → stream BytesMessage`
- Receives BytesMessage chunks from BuildKit
- Assembles into complete tar
- Writes to specified output path
- Returns empty BytesMessage on completion (acknowledgment)

Reference implementation pattern from doc lines 1245-1291 (Client Receives Tar)

### Step 3: Register FileSend Service in Session (Fixes Bug #1)
**File:** `src/buildkit/session.rs`

Around line 160-162:
```rust
// Create FileSend service for tar export
let filesend_service = FileSendService::new(output_path); // New parameter

// Add to server builder (line 178-181):
let server = Server::builder()
    .add_service(FileSyncServerBuilder::new(filesync_service))
    .add_service(FileSendServerBuilder::new(filesend_service))  // NEW
    .add_service(AuthServerBuilder::new(auth_service))
    .serve_with_incoming(conn_stream);
```

### Step 4: Advertise FileSend Method (Fixes Issue #3)
**File:** `src/buildkit/session.rs`

After line 146, add:
```rust
request.metadata_mut().append(
    "x-docker-expose-session-grpc-method",
    "/moby.filesync.v1.FileSend/DiffCopy".parse().context("Failed to parse FileSend method")?
);
```

### Step 5: Fix Tar Exporter Attrs (Fixes Issue #4)
**File:** `src/buildkit/session.rs:274-277`

Change:
```rust
attrs: [
    ("name".to_string(), image_tag.to_string()),
]
.into_iter()
.collect(),
```

To:
```rust
attrs: Default::default(),  // Tar exporter doesn't use "name"
```

### Step 6: Update BuildSession API (Fixes Issue #5)
**File:** `src/buildkit/session.rs`

Add output path parameter:
```rust
pub fn new(connection: BuildKitConnection, context_path: PathBuf, output_path: PathBuf) -> Self {
    // ...
}
```

Pass output_path to FileSendService for tar writing.

---

## Testing Verification

After implementing fixes, verify:

1. **Build with tar export works:**
   ```bash
   peelbox build --spec universalbuild.json --tag myapp:latest
   ```
   Should create tar file without errors

2. **FileSend service receives data:**
   Check logs for:
   ```
   FileSend::DiffCopy called
   Received BytesMessage chunk: N bytes
   Tar export complete: M bytes written
   ```

3. **Tar file is valid:**
   ```bash
   tar -tzf /tmp/output.tar
   docker load < /tmp/output.tar
   ```

4. **No "Unimplemented" errors** in BuildKit logs or client output

---

## Critical Files to Modify

1. ✅ `src/buildkit/proto.rs` - Add FileSend exports
2. ✅ Create `src/buildkit/filesend_service.rs` - Implement FileSend service
3. ✅ `src/buildkit/session.rs` - Register FileSend, add output path, advertise method
4. ✅ `src/buildkit/mod.rs` - Export FileSendService

---

## Root Cause Analysis

The implementation focused on **sending context to BuildKit** (FileSync) but missed the **receiving tar from BuildKit** (FileSend). This is understandable because:
- FileSync is more complex (fsutil packets with STAT/REQ/DATA/FIN protocol)
- FileSend is simpler (just BytesMessage chunks)
- Documentation emphasizes FileSync for build context
- FileSend is only mentioned in export sections

The BuildKit protocol is **bidirectional**:
- **Upload Phase:** Client serves FileSync → BuildKit downloads context
- **Export Phase:** Client serves FileSend → BuildKit uploads tar

Both must be implemented for complete workflow.

---

## References

All line numbers refer to `/tmp/buildkit/BUILDKIT_GRPC_LLB_TAR_EXPORT_FLOW.md`:
- Lines 820-870: Two Directions of File Transfer
- Lines 1143-1362: Tar Export Return Path
- Lines 473-485: Session Protocol Overview
- Lines 89-99: Exporter Configuration
- Lines 516-521: Tar Export Specifics
