# PROJECT KNOWLEDGE BASE

## OVERVIEW
peelbox is a Rust-based, deterministic BuildKit frontend for intelligent build
command detection. It statically analyzes a repository and produces distroless,
Wolfi-based container images. Detection is fully static and reproducible — there
is no LLM, no network model inference, and no API keys involved.

## STRUCTURE
```
crates/
├── core/       # Shared types: FileSystem abstraction, UniversalBuild schema
├── wolfi/      # Wolfi APKINDEX fetch + package index (two-tier cache)
├── detect/     # Map-reduce detection pipeline (parsers, framework detectors)
├── pipeline/   # DetectionService orchestration + UniversalBuild validation
├── buildkit/   # Native gRPC client & LLB generation
└── cli/        # `peelbox` binary: detect + build commands
```

## WHERE TO LOOK
| Task | Location | Notes |
|------|----------|-------|
| Add manifest parser / detector | `crates/detect/` | Self-register via `inventory::submit!` |
| Detection orchestration | `crates/pipeline/` | `DetectionService`, `Validator` |
| BuildKit / LLB | `crates/buildkit/` | Core gRPC and LLB graph builders |
| Wolfi packages | `crates/wolfi/` | APKINDEX fetch + version resolution |
| Output schema | `crates/core/` | `UniversalBuild` contract |

## CONVENTIONS
- **Deterministic Detection**: Detection is pure static analysis. No LLM, no
  network calls (beyond the cached Wolfi APKINDEX), byte-identical output.
- **Strict Distroless**: Final images must contain ZERO apk binary or metadata.
- **Merge-First**: Use `MergeOp` with independent snapshots to minimize layers.
- **Imports**: Standard library (`std::*`), then external, then crate-local.
- **Error Handling**: `anyhow::Result` with `context()` for apps; `thiserror` for core logic.
- **Documentation**: Minimalist. Code must be self-documenting. Remove "todo"/"debug" before commit.

## ANTI-PATTERNS
- **No unwrap/expect**: Use proper error handling in non-test code.
- **No backwards compatibility**: Breaking changes preferred over technical debt.
- **No manual buildctl**: Use native gRPC implementation in `crates/buildkit/`.
- **Zero Dead Code**: Always remove unused code immediately.
- **Clean Slate**: Refactor properly rather than patching.
- **No Naive Container Debugging**: When diagnosing `container_e2e` failures, NEVER run `docker` commands blindly or guess. ALWAYS:
    1. Identify the failing fixture and its `universalbuild.json`.
    2. Manually run `peelbox build --spec <path> --context <dir> --tag <tag>` for that specific context.
    3. Analyze the actual build logs (stdout/stderr) from the command to find the root cause.

## DEVELOPMENT PRINCIPLES
- **Single Responsibility**:
    - `BuildSystem`: Commands, packages, cache ONLY.
    - `Runtime`: Base images, ports, entrypoints ONLY.
    - `Framework`: Framework-specific defaults ONLY.
- **No Historical Comments**: Documentation reflects current state ONLY.

## TESTING
- **Isolated Tests**: Use a unique `PEELBOX_CACHE_DIR` per test process.
- **Serial Execution**: Sensitive tests (Docker/BuildKit) MUST use the
  `serial-tests` group in `.config/nextest.toml`.
- **Wolfi index**: container/e2e tests seed a cached `APKINDEX.tar.gz`
  (see `crates/cli/tests/data/`) so detection runs offline.

## COMMANDS
```bash
# Full local verification
cargo nextest run --release --no-default-features

# Single test
cargo nextest run <substring>

# Lint / format (CI gates)
cargo clippy --all-targets --no-default-features -- -D warnings
cargo fmt --all -- --check

# Coverage
cargo llvm-cov clean --workspace
cargo llvm-cov nextest --release --no-default-features
```
