# GitHub Actions CI/CD

This directory contains the CI/CD workflows for peelbox.

## Workflows

### CI Workflow (`.github/workflows/ci.yml`)

Runs on every push and pull request to `master`/`main` branches.

#### Jobs

**1. Test Job**
- Checks code formatting with `cargo fmt`
- Runs clippy lints
- Runs unit tests (`--lib --bins`)
- Runs integration tests in **static mode** (no LLM, fast and deterministic)
- Runs embedded LLM tests

**Key Features:**
- Uses `PEELBOX_RECORDING_MODE=replay` for deterministic LLM tests (uses pre-recorded responses from `tests/recordings/`)
- Uses `PEELBOX_DETECTION_MODE=static` for fast tests without LLM calls
- Skips container integration tests (too slow for CI)
- Caches Cargo registry, index, and build artifacts

**2. Build Docker Job** (runs only on push to master/main)
- **Builds peelbox using peelbox itself!** (dogfooding)
- Uses peelbox's BuildKit frontend to generate LLB from `universalbuild.json`
- Builds distroless Wolfi-based container image
- Tests the built image (`--version`, `--help`)
- Pushes to GitHub Container Registry at `ghcr.io/<owner>/peelbox`

**Image Tags:**
- `latest` - Latest build from master/main
- `<sha>` - Git commit SHA
- `<timestamp>` - Build timestamp

**3. Release Job** (runs only on push to master/main)
- Builds release binary with `--no-default-features` (no CUDA)
- Uploads binary as GitHub Actions artifact
- Available for download for 30 days

## LLM Testing Strategy

The CI uses a **recording/replay system** to avoid slow and non-deterministic LLM calls:

1. **Recordings** (`tests/recordings/`): Pre-recorded LLM responses captured during local test runs
2. **Replay Mode** (`PEELBOX_RECORDING_MODE=replay`): CI reads from recordings instead of making live LLM calls
3. **Static Mode** (`PEELBOX_DETECTION_MODE=static`): Uses deterministic parsers only, no LLM fallback

This approach provides:
- ✅ **Fast tests** (~2-3 minutes instead of 10-20 minutes)
- ✅ **Deterministic results** (no LLM variance)
- ✅ **No API keys needed** in CI
- ✅ **Full test coverage** (recordings verified locally before commit)

## Self-Building with BuildKit

The Docker build job demonstrates peelbox's core functionality by using it to build itself:

```bash
# 1. Build peelbox binary
cargo build --release --no-default-features

# 2. Generate BuildKit LLB from UniversalBuild spec
./target/release/peelbox frontend --spec universalbuild.json > peelbox.llb

# 3. Build container image using BuildKit
buildctl build --local context=. --output type=docker,name=peelbox:latest < peelbox.llb

# 4. Load and test the image
docker load < peelbox.tar
docker run peelbox:latest --version
```

This is a **zero-Dockerfile** build - the entire container specification is in `universalbuild.json` and processed by peelbox's BuildKit frontend!

## Running CI Locally

### Run tests like CI:
```bash
PEELBOX_RECORDING_MODE=replay PEELBOX_DETECTION_MODE=static cargo test --lib --bins
PEELBOX_RECORDING_MODE=replay PEELBOX_DETECTION_MODE=static cargo test --test e2e -- --skip test_container_integration
```

### Build Docker image like CI:
```bash
# Build peelbox
cargo build --release --no-default-features

# Start BuildKit
docker run -d --name buildkitd --privileged -p 127.0.0.1:1234:1234 moby/buildkit:latest --addr tcp://0.0.0.0:1234

# Generate LLB and build
./target/release/peelbox frontend --spec universalbuild.json > /tmp/peelbox.llb
buildctl --addr tcp://127.0.0.1:1234 build --local context=. --output type=docker,name=peelbox:test > /tmp/peelbox.tar
docker load < /tmp/peelbox.tar
docker run peelbox:test --version

# Cleanup
docker rm -f buildkitd
```

## Adding New Tests

When adding tests that use LLM:

1. Run tests locally in **record mode** first:
   ```bash
   PEELBOX_RECORDING_MODE=record cargo test <your-test>
   ```

2. Commit the generated recordings:
   ```bash
   git add tests/recordings/
   git commit -m "test: add recordings for new tests"
   ```

3. CI will automatically use recordings in replay mode

## Troubleshooting

**Tests fail in CI but pass locally:**
- Check if you have recordings committed (`tests/recordings/`)
- Verify `PEELBOX_RECORDING_MODE=replay` works locally
- Ensure no tests require local Docker/BuildKit

**Docker build fails:**
- Check BuildKit container starts successfully
- Verify `universalbuild.json` is valid
- Test `peelbox frontend` command locally

**Clippy warnings:**
- Run `cargo clippy --all-targets --all-features -- -D warnings` locally
- Fix all warnings before pushing
