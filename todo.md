# Compat Test Coverage TODO

Cross-validation of peelbox against railpack and nixpacks external fixtures.
Current state: **290 passing, 54 ignored (empty detection), 0 failing**.

## How this works

- External fixtures live in `target/external/{railpack,nixpacks}/examples/` (fetched by `./scripts/fetch-compat-fixtures.sh`)
- Committed snapshots live in `crates/cli/tests/compat-snapshots/{railpack,nixpacks}/{name}/universalbuild.json`
- An **ignored** test means peelbox produced an empty `[]` result — the language/framework was not detected at all
- A **failing** test means detection ran but hit an error (usually Wolfi package validation)

## How to fix and verify

1. Fix detection logic in `crates/detect/` (parsers, framework detectors, pipeline)
2. Regenerate the snapshot: `PEELBOX_UPDATE_COMPAT_SNAPSHOTS=1 PEELBOX_DETECTION_MODE=static cargo test -p peelbox-cli --test static_e2e -- "compat-{source}::{name}"`
3. Verify the snapshot looks correct: `cat crates/cli/tests/compat-snapshots/{source}/{name}/universalbuild.json`
4. Run full compat suite to check nothing regressed: `PEELBOX_DETECTION_MODE=static cargo test -p peelbox-cli --test static_e2e -- compat`
5. Commit the updated snapshot

---

## Ignored tests (54) — empty detection result

These fixtures produce an empty `[]` — peelbox found no recognizable manifests or language stacks. Grouped by root cause.

### Unsupported languages (13)

Languages peelbox doesn't support at all yet. Each would need a new manifest parser + optional framework detector.

- [ ] `compat-nixpacks::cobol` — COBOL (`.cbl` files)
- [ ] `compat-nixpacks::cobol-no-index` — COBOL
- [ ] `compat-nixpacks::cobol-src` — COBOL
- [ ] `compat-nixpacks::crystal` — Crystal (`shard.yml`)
- [ ] `compat-nixpacks::dart` — Dart (`pubspec.yaml`)
- [ ] `compat-nixpacks::scheme` — Scheme (`haunt.scm`)
- [ ] `compat-nixpacks::swift` — Swift (`Package.swift`)
- [ ] `compat-nixpacks::swift-custom-version` — Swift
- [ ] `compat-nixpacks::swift-vapor` — Swift (Vapor framework)
- [ ] `compat-nixpacks::basic_gleam` — Gleam (`gleam.toml`)
- [ ] `compat-railpack::gleam` — Gleam
- [ ] `compat-railpack::gleam-custom-version` — Gleam
- [ ] `compat-railpack::gleam-include-source` — Gleam

### Unsupported runtimes: Bun (2)

- [ ] `compat-railpack::node-bun` — Bun project (`bun.lockb`, `index.ts`)
- [ ] `compat-railpack::node-bun-workspaces` — Bun workspaces

### Static file / shell / config-only projects (19)

Projects that are not traditional compiled/interpreted apps — static HTML, shell scripts, config-driven builds, or env-var-only projects.

- [ ] `compat-nixpacks::staticfile` — static HTML served by a file server
- [ ] `compat-nixpacks::shell-hello` — shell script with `nixpacks.toml`
- [ ] `compat-nixpacks::apt-ffmpeg` — config-only (`nixpacks.toml` with apt packages)
- [ ] `compat-nixpacks::custom-pkgs` — config-only
- [ ] `compat-nixpacks::custom-plan-path` — config-only
- [ ] `compat-nixpacks::custom-user` — config-only
- [ ] `compat-nixpacks::pin_archive` — config-only
- [ ] `compat-nixpacks::config-from-environment-variables` — env var config only
- [ ] `compat-nixpacks::config-json-file` — JSON config only
- [ ] `compat-nixpacks::config-toml-file` — TOML config only
- [ ] `compat-railpack::railpack-env-configuration` — railpack-specific env config
- [ ] `compat-railpack::secrets` — secrets-only fixture
- [ ] `compat-railpack::dockerignore` — dockerignore-only fixture
- [ ] `compat-railpack::config-file` — railpack config file (`hello.jsonc`)
- [ ] `compat-railpack::shell-script` — shell script start command
- [ ] `compat-railpack::shell-bash-arrays` — shell script
- [ ] `compat-railpack::shell-platform-arch` — shell script
- [ ] `compat-railpack::staticfile-config` — static file serving
- [ ] `compat-railpack::staticfile-index` — static `index.html`

### Node.js detection gaps (3)

Node.js projects that produce empty results due to missing lockfile handling, missing framework detection, or tool-specific config.

#### Vite / SPA frameworks without lockfiles (1)

These have `package.json` but no lockfile, or use framework-specific config peelbox doesn't parse.

- [ ] `compat-nixpacks::node-vite-preact-ts` — Vite + Preact

#### Other Node.js issues (2)

- [ ] `compat-railpack::node-npm-workspaces` — npm workspaces
- [ ] `compat-railpack::node-yarn-workspaces` — Yarn workspaces

### Monorepo / workspace gaps (1)

- [ ] `compat-nixpacks::node-moon-monorepo` — Moon workspace tool

### Go detection gaps (4)

Go projects with only `main.go` (no `go.mod`) or multi-cmd layout.

- [ ] `compat-nixpacks::go` — single `main.go` without `go.mod`
- [ ] `compat-nixpacks::go-cgo-enabled` — CGO-enabled Go
- [ ] `compat-nixpacks::go-cmd` — Go with `cmd/` directory layout
- [ ] `compat-railpack::go-cmd-dirs` — Go with multiple cmd dirs

### Java/Gradle detection gap (2)

- [ ] `compat-railpack::java-gradle` — multi-project Gradle (app/ subdirectory)
- [ ] `compat-nixpacks::java-gradle-hello-world` — Gradle hello world

### Clojure detection gap (1)

- [ ] `compat-nixpacks::clojure-tools-build` — Clojure tools.build (not Leiningen)

### Ruby detection gap (2)

- [ ] `compat-nixpacks::ruby-2` — Ruby 2 (EOL version)
- [ ] `compat-railpack::ruby-2` — Ruby 2 (EOL version)

### Python edge cases (5)

- [ ] `compat-nixpacks::python-2` — Python 2 (`main.py` only, no manifest)
- [ ] `compat-nixpacks::python-2-runtime` — Python 2 runtime
- [ ] `compat-nixpacks::python-procfile` — Procfile-only Python project
- [ ] `compat-railpack::python-bot-only` — `bot.py` only, no manifest
- [ ] `compat-nixpacks::node-python` — mixed Node + Python (`nixpacks.toml`)

### Deno detection gaps (2)

- [ ] `compat-nixpacks::deno` — Deno without `deno.json`
- [ ] `compat-nixpacks::deno-jsonc` — Deno with `deno.jsonc` (JSONC not parsed)

---

## Priority recommendation

1. **Go/Java/Clojure/Ruby/Deno detection gaps** (11) — small targeted parser fixes.
2. **Node.js remaining gaps** (3) — Vite+Preact, npm workspaces, yarn workspaces.
3. **Unsupported languages** (13) — new parser work, lower priority unless users request.
4. **Unsupported runtimes: Bun** (2) — new runtime support needed.
5. **Static/shell/config-only** (19) — fundamentally different from build detection; may need a new detection category.
6. **Python edge cases** (5) — Python 2, Procfile-only, mixed Node+Python.
7. **Monorepo gaps** (1) — Moon workspace tool.

   Summary [1001.357s] 944 tests run: 944 passed, 164 skipped
   Summary [1585.171s] 1006 tests run: 990 passed (5 slow), 16 failed, 102 skipped
   Summary [3440.891s] 1000 tests run: 1000 passed (24 slow), 108 skipped


TODO:
 - fix strange ruby sed thingy, instead install ruby of correct versino from Gemfile.
 - cleanup pipeline.rs from language-specific stuff.
 - fix health endpoint test - we should pass healthcheck, 404 is not good enough. health endpoint should be guessed or detected correctly (although it can be / if no other page is available)
 - for some reason cache is not fully used
 - strange python venv handling with sed
 - 