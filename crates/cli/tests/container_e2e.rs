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
        "rust-custom-toolchain", // App calls `cargo version` at runtime — requires build tools in runtime image
        "node-legacy-prisma", // Prisma v2.23.0 requires OpenSSL 1.1 (libssl.so.1.1) which Wolfi doesn't provide
        "node-monorepo", // Next.js 12 + React 17 incompatible with modern Node.js (no engines/nvmrc)
        "node-pnpm-monorepo", // Next.js 12.2.5 incompatible with modern Node.js (same as node-monorepo)
        "scheme", // Wolfi's guile package (3.0.11-r6) crashes on startup — upstream packaging bug
        "node-tanstack-start", // vite build doesn't produce .output/ with locked TanStack Start ^1.95 + Vite ^7 — needs fixture version update
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
