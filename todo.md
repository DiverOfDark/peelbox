- Run `cargo fmt && cargo clippy --workspace` after each phase

// TODO later:
source_scanning - should be merged with respective static data for languages
detect/src/version -> move to respective runtimes/package managers
detect/src/postprocess -> also move to respective frameworks/languages/runtimes;
setup_commands should be merged with build commands
get rid of buildpackages and always use readily available docker image instead of wolfi for building stuff