#![allow(clippy::unnecessary_literal_unwrap)]
#![allow(clippy::type_complexity)]

mod support;

use libtest_mimic::{Arguments, Trial};
use std::collections::HashSet;
use support::discovery::{find_fixtures, Fixture};
use support::e2e::{assert_detection_with_mode, run_detection_with_mode};

fn main() {
    let args = Arguments::from_args();
    let fixtures = find_fixtures();

    let skip_fixtures: HashSet<&str> = ["multiple-manifests"].into_iter().collect();

    let mut tests = Vec::new();

    for fixture in fixtures {
        if skip_fixtures.contains(fixture.name.as_str()) {
            continue;
        }

        let expect_fail = !fixture.has_snapshot;
        let test_name = format!("{}::{}::static", fixture.category, fixture.name);

        tests.push(Trial::test(test_name, move || {
            run_test(&fixture.clone(), expect_fail)
        }));
    }

    libtest_mimic::run(&args, tests).exit();
}

fn run_test(fixture: &Fixture, expect_fail: bool) -> Result<(), libtest_mimic::Failed> {
    let mode = Some("static");
    let test_name = format!("e2e_test_{}_static", fixture.name.replace("-", "_"));

    let results_res = run_detection_with_mode(fixture.path.clone(), &test_name, mode);

    if expect_fail {
        match results_res {
            Ok(results) => {
                if !results.is_empty() {
                    return Err(format!(
                        "Expected failure/empty results for {} (no snapshot), but got {} results",
                        fixture.name,
                        results.len()
                    )
                    .into());
                }
                return Ok(());
            }
            Err(_) => {
                return Ok(());
            }
        }
    }

    let results = results_res.map_err(|e| libtest_mimic::Failed::from(e.to_string()))?;

    assert_detection_with_mode(&results, &fixture.category, &fixture.name, mode);

    Ok(())
}
