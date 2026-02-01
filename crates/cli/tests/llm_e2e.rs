#![allow(clippy::unnecessary_literal_unwrap)]
#![allow(clippy::type_complexity)]

mod support;

use libtest_mimic::{Arguments, Trial};
use support::discovery::{find_fixtures, Fixture};
use support::e2e::{assert_detection_with_mode, run_detection_with_mode};

fn main() {
    let args = Arguments::from_args();
    let fixtures = find_fixtures();

    let mut tests = Vec::new();

    for fixture in fixtures {
        if !fixture.has_snapshot {
            continue;
        }

        let fixture_clone_full = fixture.clone();
        let test_name_full = format!("{}::{}::full", fixture.category, fixture.name);

        tests.push(Trial::test(test_name_full, move || {
            run_test(&fixture_clone_full, None)
        }));

        let fixture_clone_llm = fixture.clone();
        let test_name_llm = format!("{}::{}::llm", fixture.category, fixture.name);

        tests.push(Trial::test(test_name_llm, move || {
            run_test(&fixture_clone_llm, Some("llm"))
        }));
    }

    libtest_mimic::run(&args, tests).exit();
}

fn run_test(fixture: &Fixture, mode: Option<&str>) -> Result<(), libtest_mimic::Failed> {
    let mode_suffix = mode.unwrap_or("detection");
    let test_name = format!(
        "e2e_test_{}_{}",
        fixture.name.replace("-", "_"),
        mode_suffix.replace("-", "_")
    );

    let results = run_detection_with_mode(fixture.path.clone(), &test_name, mode)
        .map_err(|e| libtest_mimic::Failed::from(e.to_string()))?;

    assert_detection_with_mode(&results, &fixture.category, &fixture.name, mode);

    Ok(())
}
