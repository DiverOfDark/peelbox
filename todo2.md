# Plan: HTTP Caching Proxy for BuildKit Integration Tests

## Context

Every container E2E test re-downloads packages (apk, npm, pip, cargo, maven) from the internet. With 100+ fixtures and 29+ container tests, this wastes significant bandwidth and time. BuildKit's protobuf defines `ProxyEnv` (http_proxy, https_proxy, ftp_proxy, no_proxy, all_proxy) but all 5 instances in `strategy.rs` hardcode `proxy_env: None`.

**Goal**: The test harness starts a caching proxy Docker container alongside BuildKit on a shared Docker network. Both containers are referenced by name. Proxy env vars are injected into all BuildKit LLB exec operations.

## Architecture

```
Host (test process)
│
├── Docker Network: peelbox-test-net
│   ├── peelbox-test-buildkit  (BuildKit daemon, port 1234 mapped to host)
│   └── peelbox-cache-proxy    (Squid with SSL bump, port 3128)
│
├── peelbox build --buildkit tcp://127.0.0.1:{port}
│   └── Reads HTTP_PROXY=http://peelbox-cache-proxy:3128
│       └── Injects into LLB pb::Meta.proxy_env
│           └── BuildKit exec ops use proxy (reachable by container name on shared net)
```

Key: BuildKit exec operations run in BuildKit's network namespace. Since BuildKit and the proxy share a Docker network, exec ops can reach `peelbox-cache-proxy:3128` by container name.

---

## Phase 1: Core Proxy Injection (code changes)

Thread `ProxyEnv` from environment → `BuildSession` → `LLBBuilder` → all `pb::Meta` structs.

### 1.1 Add `proxy_env` to `LLBBuilder`

**File: `crates/buildkit/src/llb/builder.rs`** (lines 10-18, 22-31, after line 52)

- Add field: `pub(crate) proxy_env: Option<pb::ProxyEnv>` to struct
- Initialize to `None` in `new()`
- Add builder method `with_proxy_env()` and accessor `proxy_env()`

### 1.2 Replace hardcoded `None` in strategy

**File: `crates/buildkit/src/llb/strategy.rs`**

Replace all 5 instances of `proxy_env: None` → `proxy_env: builder.proxy_env()`:
- Line 46 (apk add build packages)
- Line 84 (setup commands)
- Line 126 (build commands)
- Line 195 (runtime package install)
- Line 295 (artifact transfer)

### 1.3 Add `proxy_env` to `BuildSession`

**File: `crates/buildkit/src/session.rs`** (line 65-84, line 116-134, after line 631)

- Add field `proxy_env: Option<pb::ProxyEnv>` to `BuildSession` struct
- Initialize to `None`, add `with_proxy_env()` builder method
- Wire into LLB builder at line 627-631:
  ```rust
  if let Some(ref proxy) = self.proxy_env {
      llb_builder = llb_builder.with_proxy_env(proxy.clone());
  }
  ```

### 1.4 Environment auto-detection utility

**New file: `crates/buildkit/src/proxy.rs`**

```rust
/// Reads HTTP_PROXY/HTTPS_PROXY/NO_PROXY/FTP_PROXY/ALL_PROXY from environment.
/// Returns None if no proxy vars are set.
pub fn proxy_env_from_environment() -> Option<pb::ProxyEnv> { ... }
```

Register in `crates/buildkit/src/lib.rs` and re-export.

### 1.5 Wire into CLI

**File: `crates/cli/src/main.rs`** (after line 796)

```rust
if let Some(proxy_env) = peelbox_buildkit::proxy_env_from_environment() {
    session = session.with_proxy_env(proxy_env);
}
```

No new CLI flags — standard `HTTP_PROXY`/`HTTPS_PROXY` env vars.

---

## Phase 2: CA Cert Injection for HTTPS Caching

Most package managers use HTTPS. The SSL-bumping proxy generates a CA cert that must be trusted inside build containers.

### 2.1 Add CA cert support to LLBBuilder and BuildSession

- New fields: `proxy_ca_cert: Option<Vec<u8>>` on both `LLBBuilder` and `BuildSession`
- Builder methods: `with_proxy_ca_cert(cert: Vec<u8>)`
- Read from `PEELBOX_PROXY_CA_CERT` env var (file path) in `handle_build()`

### 2.2 Inject CA cert step in strategy

**File: `crates/buildkit/src/llb/strategy.rs`**

When `builder.proxy_ca_cert().is_some()` AND no custom `build_image`, insert a new exec step after `wolfi_base_idx`:

1. Write CA cert to `/usr/local/share/ca-certificates/proxy-ca.crt`
2. Run `update-ca-certificates` (available in Wolfi base)
3. Use resulting index as base for all subsequent steps

### 2.3 Package-manager CA trust env vars

When CA cert is present, append to build command env vars (line 113-119):
- `NODE_EXTRA_CA_CERTS=/usr/local/share/ca-certificates/proxy-ca.crt`
- `REQUESTS_CA_BUNDLE=/etc/ssl/certs/ca-certificates.crt`
- `SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt`
- `CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt`

---

## Phase 3: Test Harness Proxy Infrastructure

The proxy is managed by the test harness as a Docker container, similar to BuildKit management in `container_buildkit.rs`.

### 3.1 Docker Network Management

**New file: `crates/cli/tests/support/proxy.rs`**

Create/reuse a Docker network `peelbox-test-net`:
```rust
const PROXY_NETWORK: &str = "peelbox-test-net";
const PROXY_CONTAINER_NAME: &str = "peelbox-cache-proxy";
const PROXY_PORT: u16 = 3128;

/// Ensures the Docker network exists, returns network ID.
async fn ensure_network(docker: &Docker) -> Result<String> { ... }

/// Starts (or reuses) the caching proxy container on the shared network.
/// Returns (container_id, ca_cert_bytes).
pub async fn get_cache_proxy() -> Result<(String, Vec<u8>)> { ... }
```

Uses `OnceCell` pattern (same as `get_buildkit_container()` in `container_buildkit.rs`).

### 3.2 Static CA Certificate

Generate a test-only CA cert once and commit to the repo:

**Files (committed to git)**:
- `crates/cli/tests/data/proxy-ca/ca-cert.pem` — public CA cert (injected into builds)
- `crates/cli/tests/data/proxy-ca/ca-key.pem` — private key (used by Squid)

Generated with:
```bash
openssl req -new -newkey rsa:2048 -sha256 -days 3650 -nodes -x509 \
  -subj "/CN=peelbox-test-proxy-ca" \
  -keyout crates/cli/tests/data/proxy-ca/ca-key.pem \
  -out crates/cli/tests/data/proxy-ca/ca-cert.pem
```

This is safe to commit — it's a test-only CA never used outside Docker builds.

### 3.3 Proxy Docker Image

**Minimal Dockerfile** at `crates/cli/tests/data/proxy/Dockerfile`:
```dockerfile
FROM alpine:3.21
RUN apk add --no-cache squid openssl
EXPOSE 3128
ENTRYPOINT ["squid", "-N", "-d1"]
```

The test harness builds this image once via bollard (`docker build`). Docker caches the built image locally — subsequent runs skip the build.

Image name: `peelbox-cache-proxy:local`

### 3.4 Proxy Container Startup

The proxy container is created with bind mounts from the host:

| Host path | Container path | Purpose |
|-----------|---------------|---------|
| `tests/data/proxy-ca/ca-cert.pem` | `/etc/squid/ssl_cert/ca-cert.pem` | CA cert |
| `tests/data/proxy-ca/ca-key.pem` | `/etc/squid/ssl_cert/ca-key.pem` | CA key |
| `tests/data/proxy/squid.conf` | `/etc/squid/squid.conf` | Config |
| `target/proxy-cache/spool/` | `/var/spool/squid` | Cache data |
| `target/proxy-cache/ssl_db/` | `/var/spool/squid/ssl_db` | SSL cert DB |

**Container startup sequence**:
1. Build image if not present (`peelbox-cache-proxy:local`)
2. Create `target/proxy-cache/spool/` and `target/proxy-cache/ssl_db/` dirs
3. Create container on `peelbox-test-net` with mounts above
4. Start container
5. Exec: `/usr/lib/squid/security_file_certgen -c -s /var/spool/squid/ssl_db -M 64MB` (init SSL cert DB if empty)
6. Read CA cert bytes from `tests/data/proxy-ca/ca-cert.pem` for injection into builds

### 3.3 Attach BuildKit to Shared Network

Modify `get_buildkit_container()` in `container_buildkit.rs` (or create a similar function for E2E tests):
- After creating BuildKit container, connect it to `peelbox-test-net`:
  ```rust
  docker.connect_network(PROXY_NETWORK, ConnectNetworkOptions {
      container: &container_id,
      ..Default::default()
  }).await?;
  ```
- Keep existing port mapping so host can still reach BuildKit via `tcp://127.0.0.1:{port}`

### 3.4 Wire Proxy into Container E2E Tests

**File: `crates/cli/tests/support/container_harness.rs`** (line 50-75)

In `build_image()`, set proxy env vars on the `peelbox build` subprocess:
```rust
cmd.env("HTTP_PROXY", "http://peelbox-cache-proxy:3128");
cmd.env("HTTPS_PROXY", "http://peelbox-cache-proxy:3128");
cmd.env("NO_PROXY", "localhost,127.0.0.1");
if let Ok(ca_path) = std::env::var("PEELBOX_PROXY_CA_CERT") {
    cmd.env("PEELBOX_PROXY_CA_CERT", ca_path);
}
```

**File: `crates/cli/tests/support/e2e.rs`** (line 424)

In `run_container_integration_test()`, before building:
1. Call `proxy::get_cache_proxy()` to ensure proxy is running
2. Write CA cert to temp file
3. Set env vars for the build subprocess

### 3.5 Squid SSL Bump Configuration

**New file: `crates/cli/tests/data/proxy/squid.conf`**

```squid
# SSL bump with static CA cert
http_port 3128 ssl-bump \
  cert=/etc/squid/ssl_cert/ca-cert.pem \
  key=/etc/squid/ssl_cert/ca-key.pem \
  generate-host-certificates=on \
  dynamic_cert_mem_cache_size=64MB

sslcrtd_program /usr/lib/squid/security_file_certgen -s /var/spool/squid/ssl_db -M 64MB
acl step1 at_step SslBump1
ssl_bump peek step1
ssl_bump bump all

# Cache settings optimized for package manager traffic
maximum_object_size 512 MB
cache_dir ufs /var/spool/squid 10000 16 256
cache_mem 256 MB

# Aggressive caching for immutable package artifacts
refresh_pattern -i \.(apk|deb|rpm|tar\.gz|tgz|whl|jar|gem|crate)$ 10080 100% 43200
refresh_pattern -i /v2/.*\.(tar|tgz|tar\.gz)$ 10080 100% 43200
refresh_pattern . 60 20% 4320

# Allow all (test environment only)
http_access allow all
```

### 3.6 Persistent Cache in Cargo Target Directory

Bind-mount `target/proxy-cache/` from the host into the Squid container's cache dir (`/var/spool/squid`):
- Persists between test runs (even if container is recreated)
- Lives alongside other build artifacts — no Docker volumes needed
- Same `get_cargo_target_dir()` helper already used in test support code
- Auto-created by the proxy startup function if missing

---

## Phase 4: CI Integration

**File: `.github/workflows/ci.yml`**

The test harness manages the proxy automatically, so CI just needs:
1. Ensure Docker networking works (already does on GitHub Actions)
2. Cache the `target/proxy-cache/` directory between CI runs:
   ```yaml
   - name: Cache proxy data
     uses: actions/cache@v4
     with:
       path: target/proxy-cache
       key: proxy-cache-${{ runner.os }}
   ```

---

## Files Summary

| File | Action | Phase |
|------|--------|-------|
| `crates/buildkit/src/proxy.rs` | **Create** — `proxy_env_from_environment()` | 1 |
| `crates/buildkit/src/lib.rs` | Modify — register + re-export proxy module | 1 |
| `crates/buildkit/src/llb/builder.rs` | Modify — add `proxy_env` field + methods | 1 |
| `crates/buildkit/src/llb/strategy.rs` | Modify — 5x `None` → `builder.proxy_env()` | 1 |
| `crates/buildkit/src/session.rs` | Modify — add `proxy_env` to `BuildSession` | 1 |
| `crates/cli/src/main.rs` | Modify — call `proxy_env_from_environment()` | 1 |
| `crates/buildkit/src/llb/strategy.rs` | Modify — CA cert inject step + env vars | 2 |
| `crates/buildkit/src/session.rs` | Modify — add `proxy_ca_cert` field | 2 |
| `crates/cli/src/main.rs` | Modify — read `PEELBOX_PROXY_CA_CERT` | 2 |
| `crates/cli/tests/support/proxy.rs` | **Create** — proxy container lifecycle | 3 |
| `crates/cli/tests/support/mod.rs` | Modify — register proxy module | 3 |
| `crates/cli/tests/support/container_harness.rs` | Modify — set proxy env vars on build cmd | 3 |
| `crates/cli/tests/support/e2e.rs` | Modify — init proxy before builds | 3 |
| `crates/cli/tests/data/proxy/Dockerfile` | **Create** — minimal Alpine + Squid image | 3 |
| `crates/cli/tests/data/proxy/squid.conf` | **Create** — Squid SSL bump config | 3 |
| `crates/cli/tests/data/proxy-ca/ca-cert.pem` | **Create** — static test CA cert | 3 |
| `crates/cli/tests/data/proxy-ca/ca-key.pem` | **Create** — static test CA key | 3 |
| `.github/workflows/ci.yml` | Modify — optional cache persistence | 4 |

## Risk Notes

- **LLB cache invalidation**: `proxy_env` changes op digests. First proxied build won't reuse non-proxied cache — acceptable.
- **Custom build images**: Skip CA cert injection when `build_image` is set (user controls their own base).
- **Wolfi `update-ca-certificates`**: Available in `cgr.dev/chainguard/wolfi-base` — verified.
- **Parallel tests**: `OnceCell` ensures proxy starts once, shared across all tests (same pattern as BuildKit).

## Verification

```bash
# Phase 1 — verify no regression (no proxy):
cargo test --workspace --lib
cargo test -p peelbox-buildkit

# Phase 3 — full stack (proxy auto-starts):
PEELBOX_BUILDKIT_ADDR=tcp://127.0.0.1:1234 cargo test -p peelbox-cli --test container_e2e -- single-language::node
# Second run should be significantly faster (cached packages)
```
