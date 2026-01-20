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

    let skip_fixtures: std::collections::HashSet<&str> =
        ["multiple-manifests"].into_iter().collect();

    for fixture in fixtures {
        if !fixture.has_snapshot {
            continue;
        }

        if skip_fixtures.contains(fixture.name.as_str()) {
            continue;
        }

        let fixture_clone = fixture.clone();
        let test_name = format!("{}::{}", fixture.category, fixture.name);

        tests.push(Trial::test(test_name, move || run_test(&fixture_clone)));
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
