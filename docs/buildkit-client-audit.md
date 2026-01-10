# Audit: `feature/buildkit-client` Remaining Tasks

This document tracks technical debt, bugs, and missing features identified during the analysis of the BuildKit client implementation.

## 🔴 High Priority (Stability & Performance)
- [x] **Fix Memory Management in `FileSync`**: Refactor `read_file_chunks` to use a streaming `AsyncRead` instead of loading entire files into `Vec<Vec<u8>>` to prevent OOM on large artifacts.
- [x] **Address Async Blocking**: Remove `blocking_lock()` usage in `src/buildkit/stream_conn.rs` to prevent executor thread starvation.
- [x] **Fix Build Default Features**: Change `cuda` from a default feature to an optional one in `Cargo.toml` to allow building on standard environments without NVCC.

## 🟡 Medium Priority (Efficiency & Completeness)
- [x] **Optimize LLB Layering**: Group multiple `spec.runtime.copy` entries into a single `ExecOp` or `FileOp` to avoid creating a new container layer for every file.
- [x] **Implement Docker API Check**: Replace the placeholder in `src/buildkit/docker.rs` with actual version/BuildKit capability detection via the Docker socket.
- [x] **Extract Build Metadata**: Implement the TODO in `src/buildkit/session.rs` to extract actual layer counts from the BuildKit solve response.
- [x] **Error Handling in LLB**: Remove `2>/dev/null || true` from artifact copy commands in `LLBBuilder` to allow for proper error reporting when copies fail.

## 🟢 Low Priority (Cleanup & Docs)
- [x] **Remove Dead Code**: Delete `src/buildkit/session_bridge.rs` if it's no longer part of the architecture.
- [x] **Update README**: Update the BuildKit section to emphasize the direct `peelbox build` command instead of the `frontend | buildctl` pipe.
- [x] **Refine Gitignore Logic**: Make the hardcoded exclusions in `load_gitignore_patterns` (like `*.md`) more flexible or project-aware.
