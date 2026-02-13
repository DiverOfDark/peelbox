# peelbox-cli

CLI binary crate that wires together all other crates. Provides three commands: `detect`, `health`, and `build`.

## Module Structure

```
src/
├── main.rs              # Binary entry point + command handlers (handle_detect, handle_health, handle_build)
├── lib.rs               # Library root (re-exports cli module)
└── cli/
    ├── mod.rs           # Public API re-exports
    ├── commands.rs      # Clap argument definitions (CliArgs, DetectArgs, HealthArgs, BuildArgs)
    └── output.rs        # OutputFormatter, HealthStatus, EnvVarInfo

tests/
├── static_cli.rs        # CLI integration tests (help, version)
├── static_e2e.rs        # Static detection E2E tests (libtest-mimic, fixture discovery)
├── container_e2e.rs     # Container build tests (requires BuildKit daemon)
├── llm_e2e.rs           # LLM recording-based tests
├── llm_embedded.rs      # Embedded inference tests
├── backend_health_test.rs # Health check & config validation
├── support/             # Test utilities (e2e harness, fixture discovery, container harness)
├── fixtures/            # 100+ test fixtures (single-language, monorepo, edge-cases)
├── recordings/          # LLM request/response JSON recordings
└── data/                # Test data (APKINDEX.tar.gz)
```

## Commands

### `peelbox detect [REPOSITORY_PATH]`

Analyzes a repository and outputs `UniversalBuild` specs.

| Flag | Purpose |
|------|---------|
| `-f, --format` | json or yaml (default: json) |
| `-b, --backend` | ollama, openai, anthropic, gemini, xai, groq |
| `-m, --model` | Provider-specific model name |
| `--timeout` | Request timeout in seconds (default: 60) |
| `--no-cache` | Disable result caching |
| `-o, --output` | Write to file instead of stdout |
| `-v, --verbose` | DEBUG-level logging |
| `-q, --quiet` | Suppress non-error output |

Wraps client with `RecordingLLMClient` when `PEELBOX_ENABLE_RECORDING` is set.

### `peelbox health`

Checks backend availability for all 6 providers or a specific one.

- Ollama: HTTP GET to `/api/tags` (2s timeout)
- Others: Checks for environment variable (e.g., `OPENAI_API_KEY`)
- API keys displayed masked (`****...****`)
- Exit code 1 if any provider unavailable

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
- Skips: multiple-manifests, nested-projects, vendor-heavy

### Container E2E Tests

Require running BuildKit daemon and Docker socket access.

### LLM E2E Tests

Use recording/replay infrastructure for deterministic CI testing. Recordings stored in `tests/recordings/*.json`.

### Fixture Structure

```
tests/fixtures/
├── single-language/     # 20+ languages (rust, node, python, go, java, php, etc.)
├── monorepo/            # Cargo workspace, Gradle, Maven, npm, turborepo, polyglot
└── edge-cases/          # Multiple manifests, nested projects, empty repo, etc.
```

## Dependencies

Internal: all 5 other crates (core, llm, buildkit, wolfi -- via pipeline, detect, pipeline)
Key external: `clap` (CLI), `tokio` (async), `tracing`/`tracing-subscriber` (logging), `reqwest` (health), `atty` (terminal detection)

## Tests

Run unit tests: `cargo test -p peelbox-cli --lib`
Run static E2E: `cargo test -p peelbox-cli --test static_e2e`
Run all (needs BuildKit): `cargo test -p peelbox-cli`
