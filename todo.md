## After each phase / each language change
1. `cargo fmt`
2. `cargo clippy --workspace` — fix all warnings
3. `cargo nextest run -p peelbox-detect -p peelbox-pipeline` — affected crate tests must pass
4. Git commit the changes

# TODO: Migrate from Wolfi buildpackages to Docker Hub images

## Goal
Replace Wolfi-based builds with official Docker Hub images as the primary build/runtime base.
Fallback to `cgr.dev/chainguard/wolfi-base:latest` + `apk add` only when no suitable image exists.

## Design Principles
- **Detectors own the full build setup**: each detector is responsible for all build-time installations
- `build_image` set → detector adds `apt-get install` commands for extra deps
- Wolfi fallback → detector adds `apk add` commands for build deps
- **No build `packages` field at the end**: all build-time installations go into `commands`
- **Runtime stays unchanged**: always `cgr.dev/chainguard/glibc-dynamic:latest` + Wolfi runtime packages + build artifacts
- Wolfi resolution still needed for runtime packages only

---

## Phase 1: Pipeline — Skip build Wolfi resolution when `build_image` is set

### 1.1 Conditional skip
- **File**: `crates/detect/src/pipeline.rs` (~line 155-160)
- Skip `resolve_wolfi_packages(&mut build.build.packages)` when `build.build.build_image.is_some()`
- Keep runtime Wolfi resolution as-is (runtime always uses Wolfi)

### 1.2 Adjust version file scanning for Docker-image builds
- **File**: `crates/detect/src/pipeline.rs` (~line 1767-1829)
- When Docker images are used, version file overrides should update the image tag
- E.g., `.nvmrc` says `18` → change `build_image` from `node:22` to `node:18`

---

## Phase 2: Validation updates

### 2.1 Skip Wolfi validation for build packages when Docker image is used
- **File**: `crates/pipeline/src/validation/rules.rs`
- Skip `build.packages` validation when `build.build_image.is_some()`
- Keep runtime packages validation as-is (runtime always uses Wolfi)

---

## Phase 3: Parser migrations (one language at a time)

_Moved up — see below_

---

_Phase 3 continues here:_

## Phase 3: Parser migrations (one language at a time)

Runtime stays unchanged for all — Wolfi packages in `runtime_config.packages` are kept as-is.
Only the **build side** changes: set `build_image`, clear build `packages`, move all build-time installations to `commands`.

### Build image mapping reference

| Language | Build Image |
|----------|-------------|
| Node.js | `docker.io/library/node:{major}` |
| Python | `docker.io/library/python:{major.minor}` |
| Rust | `docker.io/library/rust:{version}` |
| Go | `docker.io/library/golang:{version}` |
| Java/Maven | `docker.io/library/maven:{maven}-eclipse-temurin-{jdk}` |
| Java/Gradle | `docker.io/library/gradle:{gradle}-jdk{jdk}` |
| Ruby | `docker.io/library/ruby:{version}` |
| PHP | `docker.io/library/php:{version}-cli` |
| .NET | `mcr.microsoft.com/dotnet/sdk:{version}` |
| Swift | Already `docker.io/library/swift:{version}` |
| Elixir | `docker.io/library/elixir:{version}` |
| Zig, C/C++, Dart | Wolfi fallback (no official image) |

### 4.1 Swift (already done, minimal change)
- **File**: `crates/detect/src/parsers/manifest/package_swift.rs`
- Already sets `build_image`. Just verify `packages` is empty when `build_image` is set.

### 4.2 Rust
- **File**: `crates/detect/src/parsers/manifest/cargo_toml.rs`
- Set `build_image: Some(format!("docker.io/library/rust:{}", version))`
- Clear build `packages` (image has rustc, cargo)
- If native deps needed (openssl-dev), prepend `apt-get install -y --no-install-recommends libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*` to `commands`
- Remove `resolve_rust_toolchain()` old-version workaround (Docker has all versions)
- Keep `runtime_config.packages` unchanged (Wolfi runtime packages)

### 4.3 Go
- **Files**: `crates/detect/src/parsers/manifest/go_mod.rs`, `go_work.rs`, `go_main.rs`
- Set `build_image: Some(format!("docker.io/library/golang:{}", version))`
- Clear build `packages`
- If `CGO_ENABLED=1`, add `apt-get install build-essential` to commands
- Keep `runtime_config.packages` unchanged

### 4.4 Node.js
- **File**: `crates/detect/src/parsers/manifest/package_json.rs`
- Set `build_image: Some(format!("docker.io/library/node:{}", node_major))`
- Clear build `packages` (image has node, npm)
- For native extensions (node-gyp), add `apt-get install -y build-essential python3` to commands
- Remove the `n` installer workaround for old Node versions (Docker has them all)
- Keep `runtime_config.packages` unchanged
- **Note**: Bun has no official Docker Hub image → keep on Wolfi fallback for now

### 4.5 Python
- **Files**: `crates/detect/src/parsers/manifest/pyproject_toml.rs`, `requirements_txt.rs`, `pipfile.rs`, `setup_py.rs`, `pdm_lock.rs`, `uv_lock.rs`, `environment_yml.rs`, `python_script.rs`
- Set `build_image: Some(format!("docker.io/library/python:{}", version))`
- Clear build `packages` (image has python, pip)
- For native deps (libpq, cairo), add apt-get install commands
- Poetry/uv: `pip install poetry` or `pip install uv` in commands
- Keep `runtime_config.packages` unchanged

### 4.6 Java — Maven
- **File**: `crates/detect/src/parsers/manifest/pom_xml.rs`
- Set `build_image: Some(format!("docker.io/library/maven:3-eclipse-temurin-{}", jdk_version))`
- Clear build `packages`
- Remove `resolve_java_toolchain()` old-version workaround
- Keep `runtime_config.packages` unchanged

### 4.7 Java — Gradle
- **File**: `crates/detect/src/parsers/manifest/build_gradle.rs`
- Set `build_image: Some(format!("docker.io/library/gradle:{}-jdk{}", gradle_version, jdk_version))`
- Clear build `packages`
- Keep `runtime_config.packages` unchanged

### 4.8 Ruby
- **File**: `crates/detect/src/parsers/manifest/gemfile.rs`
- Set `build_image: Some(format!("docker.io/library/ruby:{}", version))`
- Clear build `packages`
- For native gems (nokogiri), add `apt-get install -y libxml2-dev libxslt1-dev` to commands
- Keep `runtime_config.packages` unchanged

### 4.9 .NET
- **File**: `crates/detect/src/parsers/manifest/csproj.rs`
- Set `build_image: Some(format!("mcr.microsoft.com/dotnet/sdk:{}", version))`
- Clear build `packages`
- Keep `runtime_config.packages` unchanged

### 4.10 PHP
- **File**: `crates/detect/src/parsers/manifest/composer_json.rs`
- Set `build_image: Some(format!("docker.io/library/php:{}-cli", version))`
- PHP extensions: use `docker-php-ext-install` in commands instead of Wolfi packages
  - E.g., `docker-php-ext-install pdo_mysql pdo_pgsql` instead of `php-8.3-pdo_mysql`
- Clear build `packages`
- Keep `runtime_config.packages` unchanged
- **Note**: Most complex migration due to extension system difference

### 4.11 Elixir
- **File**: `crates/detect/src/parsers/manifest/mix_exs.rs`
- Set `build_image: Some(format!("docker.io/library/elixir:{}", version))`
- Clear build `packages` (image has elixir + erlang)
- Keep `runtime_config.packages` unchanged

### 3.12 Wolfi fallback languages (build packages → commands)
These keep `build_image: None` (wolfi-base) but must move build `packages` into `commands` as `apk add --no-cache ...`:
- `zig_build.rs`, `build_zig_zon.rs` (Zig)
- `cmake_lists.rs`, `makefile.rs`, `meson_build.rs` (C/C++)
- `pubspec_yaml.rs` (Dart/Flutter)
- `cabal_file.rs`, `stack_yaml.rs` (Haskell)
- `build_sbt.rs` (Scala/SBT)
- `deps_edn.rs`, `project_clj.rs` (Clojure)
- `gleam_toml.rs` (Gleam)
- `shard_yml.rs` (Crystal)
- `scheme.rs`, `cobol.rs`

---

## Phase 4: Remove `packages` from build schema

### 4.1 Remove `packages` from `BuildSpec`
- **File**: `crates/detect/src/types.rs`
- Remove `pub packages: Vec<String>` from `BuildSpec`
- All build-time package installations are now in `commands`

### 4.2 Remove `packages` from `BuildStage`
- **File**: `crates/core/src/output/schema.rs`
- Remove `pub packages: Vec<String>` from `BuildStage`

### 4.3 Remove build package installation from BuildKit strategy
- **File**: `crates/buildkit/src/llb/strategy.rs` (~line 33-67)
- Remove the `apk add --no-cache {packages}` exec step for build packages
- Build packages are now installed via `commands` by each detector

### 4.4 Remove build-side Wolfi resolution entirely
- **File**: `crates/detect/src/pipeline.rs`
- Remove `resolve_wolfi_packages(&mut build.build.packages)` call entirely
- Remove build-side entries from `VERSIONABLE_PACKAGES`
- Remove old-version workarounds (Node via `n`, Rust via rustup, Java via Adoptium)
- Keep all runtime-related Wolfi resolution intact

### 4.5 Remove build package validation
- **File**: `crates/pipeline/src/validation/rules.rs`
- Remove build package validation from `validate_wolfi_packages()`
- Keep runtime package validation

### 4.6 Remove build-side native dep scanning
- Functions like `scan_python_native_deps`, `scan_node_native_deps` that add Wolfi build packages
- Replaced by apt-get/apk commands added directly by the parsers
- Keep runtime native dep scanning unchanged

### 4.7 Scope down `crates/wolfi/` usage
- No longer needed for build packages at all
- Keep for runtime packages (runtime always uses Wolfi)
- APKINDEX download + caching still needed for runtime

---

## Phase 5: Test updates

### 5.1 Parser unit tests (~80+ tests across 10+ files)
- Verify `build_image` is set for Docker-image languages
- Verify build commands include apt-get/apk install steps
- Verify no build `packages` field (after Phase 4)
- Verify `runtime_config.packages` unchanged

### 5.2 Pipeline integration tests
- Verify Wolfi resolution no longer runs for build packages
- Verify Wolfi resolution still runs for runtime packages
- Verify version file overrides update Docker image tags

### 5.3 E2E fixture tests (29 fixtures in `tests/`)
- Update expected output JSON: no build `packages`, has `build_image`
- Keep expected Wolfi runtime packages

---

## Recommended execution order
1. Phase 1 (pipeline) — conditional build Wolfi resolution skip
2. Phase 2 (validation) — skip build validation for Docker-image builds
3. Phase 3 (parsers) — one language at a time, each independently shippable:
   - 3.1 Swift (verify), 3.2 Rust, 3.3 Go, 3.4 Node, 3.5 Python
   - 3.6 Maven, 3.7 Gradle, 3.8 Ruby, 3.9 .NET, 3.10 PHP, 3.11 Elixir
   - 3.12 Wolfi fallback languages (move packages → commands)
4. Phase 4 (remove build packages) — after ALL parsers migrated, remove `packages` from BuildSpec/BuildStage/strategy
5. Phase 5 (tests) — ongoing, update as each parser migrates
