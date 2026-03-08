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
        "node-legacy-prisma",    // Prisma v2.23.0 requires OpenSSL 1.1 (libssl.so.1.1) which Wolfi doesn't provide
        "node-monorepo",         // Next.js 12 + React 17 incompatible with modern Node.js (no engines/nvmrc)
        "node-pnpm-monorepo",    // Next.js 12.2.5 incompatible with modern Node.js (same as node-monorepo)
    ]
    .into_iter()
    .collect();

    // Tests identified by "category::name" that need skipping for category-specific reasons
    let skip_full: std::collections::HashSet<&str> = [
        "compat-nixpacks::node-puppeteer", // App navigates to external URL — requires network access
        "compat-nixpacks::python-django-mysql", // Django runserver crashes when MySQL socket unavailable (check_migrations fails)
        "compat-nixpacks::python-setuptools", // Flask app.run() binds to 127.0.0.1 — unreachable from outside container
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
        if skip_full.contains(test_name.as_str()) {
            continue;
        }

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
