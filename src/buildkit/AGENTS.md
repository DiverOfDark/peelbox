# BUILDKIT MODULE KNOWLEDGE BASE

## OVERVIEW
Specialized gRPC client and LLB graph generator for native BuildKit interaction. Replaces `buildctl`.

## STRUCTURE
```
src/buildkit/
├── connection.rs   # gRPC endpoint discovery & handshake
├── llb.rs          # LLB protobuf graph construction
├── session.rs      # Build session orchestration
├── progress.rs     # Stream processing for build events
└── services/       # FileSync, FileSend, Auth, Health
```

## WHERE TO LOOK
| Task | Location | Logic |
|------|----------|-------|
| Graph logic | `src/buildkit/llb.rs` | SourceOp, ExecOp, MergeOp orchestration |
| Connection | `src/buildkit/connection.rs` | HTTP upgrade for Docker socket |
| Streaming | `src/buildkit/progress.rs` | Vertex tracking & text-based logging |
| Artifacts | `src/buildkit/filesend_service.rs` | Exfiltration of build outputs |

## CONVENTIONS
- **Strict Distroless**: Every build must use the 4-stage squash strategy.
- **Merge-First**: Favor `MergeOp` over large `ExecOp` copies to minimize layers.
- **Native gRPC**: Stick to BuildKit v0.12.5 protobuf definitions.
- **Socket Handshake**: Use `POST /grpc` for Docker daemon connections.

## ANTI-PATTERNS
- **CLI Dependency**: Never shell out to `buildctl`.
- **Whiteout Leaks**: Avoid modification layers that leave `apk` traces.
- **Blocking Logs**: Progress must be streamed asynchronously.
