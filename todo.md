# Compat Test Coverage TODO

Cross-validation of peelbox against railpack and nixpacks external fixtures.
Current state: **154 passing, 90 ignored (empty detection), 13 failing (detection errors)**.

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

## Failing tests (13) — detection errors

These fixtures are detected but hit errors during Wolfi package validation or pipeline processing. Fix the detection/version logic so valid packages are emitted, then regenerate snapshots.

### Unsupported old Go versions (3)

Peelbox resolves Go versions from `go.mod` but Wolfi doesn't have packages for Go <1.20. Detection should either clamp to the oldest available version or gracefully handle EOL versions.

- [ ] `compat-nixpacks::go-gin` — `go-1.18` not found (go.mod says 1.18)
- [ ] `compat-nixpacks::go-mod` — `go-1.17` not found (go.mod says 1.17)
- [ ] `compat-nixpacks::go-custom-version` — `go-1.18` not found (go.mod says 1.18)

### Unsupported old .NET versions (5)

These fixtures target .NET 6 which is EOL and not in Wolfi. Detection should clamp to the oldest supported version or handle gracefully.

- [ ] `compat-nixpacks::csharp-api` — `aspnet-6-runtime` not found
- [ ] `compat-nixpacks::csharp-cli` — `aspnet-6-runtime` not found
- [ ] `compat-nixpacks::fsharp-api` — `aspnet-6-runtime` not found
- [ ] `compat-nixpacks::fsharp-cli` — `aspnet-6-runtime` not found
- [ ] `compat-railpack::dotnet-cli` — `aspnet-6-runtime` not found

### Unsupported old Java versions (3)

Java 1.8 and 1.x version format is not mapped correctly to Wolfi package names (`openjdk-1.8` should map to `openjdk-8` or similar).

- [ ] `compat-nixpacks::java-maven` — `openjdk-1.8` not found (pom.xml targets Java 8)
- [ ] `compat-nixpacks::java-maven-wrapper` — `openjdk-1.8` not found (pom.xml targets Java 8)
- [ ] `compat-nixpacks::java-spring-boot-1` — `openjdk-1` not found (Spring Boot 1.x, Java 1.x)

### Unsupported old Node.js version (1)

`.nvmrc` specifies Node 14 which is EOL and not in Wolfi.

- [ ] `compat-nixpacks::node-nvmrc` — `nodejs-14` not found (.nvmrc says 14.19.3)

### Duplicate service name bug (1)

Pipeline produces two services both named `app`, causing a uniqueness check failure. Likely a multi-manifest or config interaction issue.

- [ ] `compat-nixpacks::php-custom-config` — "Duplicate service names detected: app"

---

## Ignored tests (90) — empty detection result

These fixtures produce an empty `[]` — peelbox found no recognizable manifests or language stacks. Grouped by root cause.

### Unsupported languages (24)

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

### Unsupported runtimes: Bun (5)

Bun is detected as a package manager but these fixtures use Bun-specific project structures that aren't fully handled.

- [ ] `compat-railpack::bun-pnpm` — Bun project with pnpm lockfile
- [ ] `compat-railpack::node-bun` — Bun project (`bun.lockb`, `index.ts`)
- [ ] `compat-railpack::node-bun-workspaces` — Bun workspaces
- [ ] `compat-nixpacks::node-bun-web-server` — (this one passes, but listed for reference if related issues arise)

### Static file / shell / config-only projects (12)

Projects that are not traditional compiled/interpreted apps — static HTML, shell scripts, config-driven builds, or env-var-only projects.

- [ ] `compat-nixpacks::staticfile` — static HTML served by a file server
- [ ] `compat-nixpacks::shell-hello` — shell script with `nixpacks.toml`
- [ ] `compat-nixpacks::apt-ffmpeg` — config-only (`nixpacks.toml` with apt packages)
- [ ] `compat-nixpacks::custom-pkgs` — config-only
- [ ] `compat-nixpacks::custom-plan-path` — config-only
- [ ] `compat-nixpacks::custom-user` — config-only
- [ ] `compat-nixpacks::pin_archive` — config-only
- [ ] `compat-nixpacks::overriding-environment-variables` — env var config only
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

### Node.js detection gaps (27)

Node.js projects that are detected but produce empty results due to missing lockfile handling, missing framework detection, or tool-specific config.

#### Yarn Berry / Yarn 1 / Yarn workspaces (7)

- [ ] `compat-nixpacks::node-yarn-berry` — Yarn Berry (`.yarnrc.yml`)
- [ ] `compat-nixpacks::node-yarn-prisma` — Yarn with Prisma
- [ ] `compat-railpack::node-yarn-1` — Yarn 1 Classic
- [ ] `compat-railpack::node-yarn-2` — Yarn 2
- [ ] `compat-railpack::node-yarn-2-node-linker` — Yarn 2 with node-linker
- [ ] `compat-railpack::node-yarn-3` — Yarn 3
- [ ] `compat-railpack::node-yarn-workspaces` — Yarn workspaces

#### Vite / SPA frameworks without lockfiles (13)

These have `package.json` but no lockfile, or use framework-specific config peelbox doesn't parse.

- [ ] `compat-nixpacks::node-vite-lit-ts` — Vite + Lit (TypeScript)
- [ ] `compat-nixpacks::node-vite-preact-ts` — Vite + Preact
- [ ] `compat-nixpacks::node-vite-qwik-ts` — Vite + Qwik
- [ ] `compat-nixpacks::node-vite-react-ts` — Vite + React (TypeScript)
- [ ] `compat-nixpacks::node-vite-solid-ts` — Vite + Solid
- [ ] `compat-nixpacks::node-vite-svelte-ts` — Vite + Svelte (TypeScript)
- [ ] `compat-nixpacks::node-vite-vanilla-ts` — Vite vanilla TypeScript
- [ ] `compat-nixpacks::node-vite-vue-ts` — Vite + Vue (TypeScript)
- [ ] `compat-railpack::node-vite-react` — Vite + React
- [ ] `compat-railpack::node-vite-react-router-spa` — Vite + React Router SPA
- [ ] `compat-railpack::node-vite-react-router-ssr` — Vite + React Router SSR
- [ ] `compat-railpack::node-vite-svelte` — Vite + Svelte
- [ ] `compat-railpack::node-vite-vanilla` — Vite vanilla

#### Other Node.js issues (7)

- [ ] `compat-nixpacks::node-no-scripts` — `package.json` without scripts section
- [ ] `compat-nixpacks::node-main-file` — `package.json` with `main` field but no start script
- [ ] `compat-nixpacks::node-main-file-not-exist` — `main` field points to non-existent file
- [ ] `compat-nixpacks::node-custom-cache-directories` — custom cache config
- [ ] `compat-nixpacks::node-legacy-prisma` — old Prisma version
- [ ] `compat-nixpacks::node-react-router-v7-spa` — React Router v7 SPA mode
- [ ] `compat-nixpacks::node-variables` — env variable usage in build
- [ ] `compat-nixpacks::node-typescript-incremental` — TypeScript incremental builds
- [ ] `compat-nixpacks::node-typescript-incremental-extends` — TypeScript extends
- [ ] `compat-nixpacks::node-typescript-incremental-out-dir` — TypeScript outDir
- [ ] `compat-nixpacks::node-typescript-incremental-tsbuildinfo-path` — tsBuildInfoFile path
- [ ] `compat-railpack::node-astro` — Astro (no lockfile)
- [ ] `compat-railpack::node-astro-server` — Astro SSR
- [ ] `compat-railpack::node-nuxt` — Nuxt (no lockfile or missing detection)
- [ ] `compat-railpack::node-puppeteer` — Puppeteer (system deps needed)
- [ ] `compat-railpack::node-svelte-kit` — SvelteKit (no lockfile)
- [ ] `compat-railpack::node-npm-workspaces` — npm workspaces

### Monorepo / workspace gaps (3)

- [ ] `compat-nixpacks::node-moon-monorepo` — Moon workspace tool
- [ ] `compat-nixpacks::node-nx-20` — Nx 20 workspace
- [ ] `compat-nixpacks::nested` — nested project structure

### Go detection gaps (3)

Go projects with only `main.go` (no `go.mod`) or multi-cmd layout.

- [ ] `compat-nixpacks::go` — single `main.go` without `go.mod`
- [ ] `compat-nixpacks::go-cgo-enabled` — CGO-enabled Go
- [ ] `compat-nixpacks::go-cmd` — Go with `cmd/` directory layout
- [ ] `compat-railpack::go-cmd-dirs` — Go with multiple cmd dirs

### Java/Gradle detection gap (2)

- [ ] `compat-railpack::java-gradle` — multi-project Gradle (app/ subdirectory)
- [ ] `compat-nixpacks::java-gradle-hello-world` — Gradle hello world

### Python edge cases (4)

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

1. **Failing tests first** (13) — these are regressions-adjacent; detection runs but crashes on version mapping. Fix Wolfi package version clamping for Go, .NET, Java, Node.
2. **Node.js Vite/lockfile gaps** (20+) — high-impact, covers popular frameworks, likely a few parser fixes.
3. **Bun support** (5) — growing ecosystem, relatively isolated parser work.
4. **Unsupported languages** (13) — new parser work, lower priority unless users request.
5. **Static/shell/config-only** (20) — fundamentally different from build detection; may need a new detection category.


Before:
Summary [1590.351s] 861 tests run: 755 passed (3 slow), 106 failed, 180 skipped
Summary [1435.785s] 834 tests run: 745 passed (4 slow), 89 failed, 114 skipped
Summary [1220.253s] 900 tests run: 796 passed (2 slow), 104 failed, 180 skipped
Summary [1347.700s] 866 tests run: 758 passed (4 slow), 108 failed, 131 skipped
Summary [1440.954s] 899 tests run: 738 passed (5 slow), 161 failed, 180 skipped
Summary [1439.741s] 910 tests run: 744 passed (4 slow), 166 failed, 180 skipped
Summary [2365.828s] 948 tests run: 895 passed (11 slow), 53 failed, 164 skipped
Summary [2653.454s] 949 tests run: 914 passed (18 slow), 35 failed, 164 skipped
Summary [1372.206s] 891 tests run: 806 passed (2 slow), 85 failed, 135 skipped

TODO:
 - fix strange ruby sed thingy, instead install ruby of correct versino from Gemfile.
 - cleanup pipeline.rs from language-specific stuff.
 - fix health endpoint test - we should pass healthcheck, 404 is not good enough. health endpoint should be guessed or detected correctly (although it can be / if no other page is available)
 - for some reason cache is not fully used
 - strange python venv handling with sed
 - 