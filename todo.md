# Compat Test Coverage TODO

Cross-validation of peelbox against railpack and nixpacks external fixtures.
Current state: **344 passing, 0 ignored, 0 failing**.

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
