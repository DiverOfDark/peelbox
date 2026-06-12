# peelbox-cli

CLI binary crate that wires together the other crates. Provides two commands:
`detect` and `build`.

## Module Structure

```
src/
├── main.rs              # Binary entry point + command handlers (handle_detect, handle_build)
├── lib.rs               # Library root (re-exports cli module)
└── cli/
    ├── mod.rs           # Public API re-exports
    ├── commands.rs      # Clap argument definitions (CliArgs, DetectArgs, BuildArgs)
    └── output.rs        # OutputFormatter (JSON/YAML)

tests/
├── static_cli.rs        # CLI surface tests (help, version, error paths)
├── static_e2e.rs        # Static detection E2E tests (libtest-mimic, fixture discovery)
├── container_e2e.rs     # Container build tests (requires BuildKit daemon)
├── container_buildkit.rs # BuildKit daemon integration tests
├── support/             # Test utilities (e2e harness, fixture discovery, container harness)
├── fixtures/            # 100+ test fixtures (single-language, monorepo, edge-cases)
└── data/                # Test data (APKINDEX.tar.gz)
```

## Commands

### `peelbox detect [REPOSITORY_PATH]`

Statically analyzes a repository and outputs `UniversalBuild` specs. Detection
is deterministic — no backend, no API key, no network LLM calls.

| Flag | Purpose |
|------|---------|
| `-f, --format` | json or yaml (default: json) |
| `-o, --output` | Write to file instead of stdout |
| `-v, --verbose` | DEBUG-level logging |
| `-q, --quiet` | Suppress non-error output |

### `peelbox build --spec <FILE> --tag <TAG>`

Builds container images from UniversalBuild specs using BuildKit.

| Flag | Purpose |
|------|---------|
| `--spec` | Path to UniversalBuild JSON (single or array) |
| `--tag` | Image tag (e.g., myapp:latest) |
| `--output` | docker (default), oci, type=oci,dest=file.tar |
| `--buildkit` | BuildKit daemon address |
| `--service` | Service name for monorepo specs |
| `--context` | Build context directory (default: cwd) |
| `--sbom` / `--no-sbom` | SBOM attestation (default: enabled) |
| `--provenance` | min or max (default: max) |
| `--cache` | Cache config (repeatable): user/app:cache, type=local,src=dir |

Cache handling:
- Auto-cache from `PEELBOX_CACHE_DIR` with SHA256-derived key
- Supports: registry, local, gha, s3, azblob, inline
- Auto-resolves digest from OciIndex for local caches

## Testing Infrastructure

### Static E2E Tests (`static_e2e.rs`)

Uses `libtest-mimic` custom harness for dynamic fixture discovery:
- Discovers fixtures from `tests/fixtures/`
- Compares detection output against `universalbuild.json` snapshots

### Container E2E Tests

Require a running BuildKit daemon and Docker socket access.

### Fixture Structure

```
tests/fixtures/
├── single-language/     # 20+ languages (rust, node, python, go, java, php, etc.)
├── monorepo/            # Cargo workspace, Gradle, Maven, npm, turborepo, polyglot
└── edge-cases/          # Multiple manifests, nested projects, empty repo, etc.
```

## Dependencies

Internal: `peelbox-core`, `peelbox-pipeline`, `peelbox-buildkit`
Key external: `clap` (CLI), `tokio` (async), `tracing`/`tracing-subscriber` (logging)

## Tests

Run unit tests: `cargo test -p peelbox-cli --lib`
Run static E2E: `cargo test -p peelbox-cli --test static_e2e`
Run all (needs BuildKit): `cargo test -p peelbox-cli`
