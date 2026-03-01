# Compat Snapshots

This directory contains peelbox-native `universalbuild.json` snapshots for fixtures from external projects (railpack, nixpacks). These snapshots enable cross-validation testing using our existing E2E infrastructure.

## Structure

```
compat-snapshots/
├── railpack/
│   ├── node-npm/universalbuild.json
│   ├── python-pip/universalbuild.json
│   └── ...
├── nixpacks/
│   ├── node-npm/universalbuild.json
│   └── ...
└── README.md
```

Each `universalbuild.json` is **peelbox's own detection output** for the corresponding external fixture. Only fixtures with a committed snapshot are tested — curation happens at generation time.

## Workflow

### Fetching external fixtures

```bash
./scripts/fetch-compat-fixtures.sh
```

This sparse-clones the `examples/` directories from railpack and nixpacks into `target/external/`. Repos and commits are pinned in `compat-sources.toml`.

### Generating / updating snapshots

```bash
# Generate snapshots for all external examples
PEELBOX_UPDATE_COMPAT_SNAPSHOTS=1 cargo test -p peelbox-cli --test static_e2e -- compat

# Review generated snapshots
git diff crates/cli/tests/compat-snapshots/

# Commit only the ones peelbox handles correctly
git add crates/cli/tests/compat-snapshots/
```

### Running compat tests

```bash
# All tests (including compat)
cargo test -p peelbox-cli --test static_e2e

# Only compat tests
cargo test -p peelbox-cli --test static_e2e -- compat

# Container tests (requires BuildKit)
cargo test -p peelbox-cli --test container_e2e -- compat
```

## How it works

1. `find_fixtures()` discovers compat fixtures alongside regular ones
2. Working directories are assembled in `target/compat-work/` by copying external project files + the committed snapshot
3. The existing static and container E2E infrastructure tests them unchanged

If `target/external/` doesn't exist (script not run), compat tests are silently absent — existing tests still pass.
