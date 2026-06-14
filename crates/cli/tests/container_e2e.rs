#![allow(clippy::unnecessary_literal_unwrap)]
#![allow(clippy::type_complexity)]

mod support;

use libtest_mimic::{Arguments, Trial};
use support::discovery::{find_fixtures, Fixture};
use support::e2e::run_container_integration_test;

fn main() {
    let args = Arguments::from_args();
    let fixtures = find_fixtures();

    let mut tests = Vec::new();

    let skip_fixtures: std::collections::HashSet<&str> = [
        "multiple-manifests",
        "rust-custom-toolchain", // Calls `cargo version` at runtime — cargo not in runtime image
        "node-legacy-prisma", // Prisma v2.23.0 requires OpenSSL 1.1 (libssl.so.1.1) which Wolfi doesn't provide
        "node-monorepo", // Next.js 12 + React 17 — no engines/nvmrc so detector picks latest Node which is incompatible
        "node-pnpm-monorepo", // Turborepo shared tsconfig not resolved — build fails with tsconfig/nextjs.json not found
        "scheme", // Wolfi's guile package (3.0.11-r6) crashes on startup — upstream packaging bug
        // Dummy/non-executable commands — test config parsing, not runtime
        "config-json-file",
        "custom-plan-path",
        // Runs forever with --no-halt, no port, no stdout to validate
        "elixir-ecto",
        "elixir-latest",
        // Requires external PostgreSQL database
        "python-postgres",
        "python-psycopg",
        "python-psycopg2",
        // Python 2 EOL — not available as Wolfi package
        "python-2",
        "python-2-runtime",
        // Non-deterministic output (versions, random data, architecture)
        "node-python",
        "python-numpy",
        "apt-ffmpeg",
        "shell-platform-arch",
        // Requires runtime env vars or secrets not injected during test
        "config-from-environment-variables",
        "config-toml-file",
        "secrets",
        // Tests build-time behavior, not runtime execution
        "dockerignore",
        "railpack-env-configuration",
        // Shell scripts without shebang — exec format error in exec-form entrypoint
        "shell-hello",
        "custom-pkgs",
        "pin_archive",
        "shell-script",
        "node-custom-version",
        // Build needs git for clojure tools-build dependency resolution
        "clojure-tools-build",
        // App asserts PATH starts with /app/.venv/bin — venv not configured in runtime
        "python-pip",
        // COBOL — Wolfi gnucobol crashes on startup
        "cobol",
        "cobol-no-index",
        "cobol-src",
        // Zig source scanner doesn't detect ports — app listens on 8080 but snapshot has []
        "zig-plain",
        // Upstream example pins no fasthtml version; current python-fasthtml
        // removed the `picolink`/`gridlink` exports the app imports, so it crashes
        // at runtime (NameError: name 'picolink' is not defined). Not a detection
        // issue — the example app is incompatible with the released library.
        "python-fasthtml",
    ]
    .into_iter()
    .collect();

    for fixture in fixtures {
        if !fixture.has_snapshot {
            continue;
        }

        if skip_fixtures.contains(fixture.name.as_str()) {
            continue;
        }

        let test_name = format!("{}::{}", fixture.category, fixture.name);
        let ignored = fixture.ignore;
        let fixture_clone = fixture.clone();

        let trial =
            Trial::test(test_name, move || run_test(&fixture_clone)).with_ignored_flag(ignored);
        tests.push(trial);
    }

    libtest_mimic::run(&args, tests).exit();
}

fn run_test(fixture: &Fixture) -> Result<(), libtest_mimic::Failed> {
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

    runtime.block_on(async {
        run_container_integration_test(&fixture.category, &fixture.name)
            .await
            .map_err(|e| libtest_mimic::Failed::from(e.to_string()))
    })
}
