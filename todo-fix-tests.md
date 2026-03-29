# Container E2E Test Failures — Current Status

## Passing (12/22)
- `compat-nixpacks::basic_gleam` (9.4s)
- `compat-nixpacks::scheme` (2.3s)
- `compat-nixpacks::node` (11s)
- `compat-nixpacks::node-main-file-not-exist` (13s)
- `compat-nixpacks::node-typescript-incremental-out-dir` (12.5s)
- `compat-nixpacks::ruby-3` (9.4s)
- `compat-nixpacks::staticfile` (1.9s)
- `compat-railpack::gleam` (9.2s)
- `compat-railpack::gleam-custom-version` (7.3s)
- `compat-railpack::config-file` (16.7s)
- `compat-railpack::staticfile-index` (1.8s)
- `compat-railpack::staticfile-config` (1.9s)

## Still Failing (10/22)

### Go (2 tests) — snapshot mismatch after `build_image` field addition
- `compat-nixpacks::go` (1.9s)
- `compat-nixpacks::go-cgo-enabled` (1.9s)

### Crystal (1 test) — likely snapshot mismatch
- `compat-nixpacks::crystal` (2.2s)

### Python Procfile (1 test) — snapshot or build issue
- `compat-nixpacks::python-procfile` (1.6s)

### Swift (3 tests) — build_image not fully integrated
- `compat-nixpacks::swift` (2.5s)
- `compat-nixpacks::swift-custom-version` (2.1s)
- `compat-nixpacks::swift-vapor` (9.1s)

### Gleam include-source (1 test) — escript/erlang issue
- `compat-railpack::gleam-include-source` (1.4s)

### Node monorepo (2 tests) — npm start missing script
- `compat-railpack::node-npm-workspaces` (69.4s)
- `compat-nixpacks::node-moon-monorepo` (103.7s)

## Key Fix Applied
- **Docker export hang resolved**: `session.rs` now skips tar stream wait for Docker native connections with `type=docker` output. This was the root cause of all previous timeouts.
