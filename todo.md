# aipack Cleanup, Refactor & Bug Fixes

## Phase 1: Dead Code Removal & Trivial Cleanup
**Risk: Lowest | Snapshot updates: 0**

- [x] Remove unused `_extra_packages` param in `nixpacks_config.rs` `build_config_manifest()`
- [x] Clean `#[allow(dead_code)]` in `compat_discovery.rs` (5 functions)
- [x] Remove empty `skip_full` HashSet in `container_e2e.rs`
- [x] Fix placeholder `echo 'Config-only project'` build command in `nixpacks_config.rs`
- [x] Fix formatting in `bazel_build.rs` around `build_image: None`
- [x] Fix `_repo_root` → `repo_root` in `pipeline.rs` `scan_node_puppeteer` (it IS used)

## Phase 2: Deduplicate Pattern Tables
**Risk: Low | Snapshot updates: 0**

- [ ] Merge duplicate JS/TS entries in `PORT_PATTERNS`, `HEALTH_PATTERNS`, `ENV_VAR_PATTERNS`
- [ ] Merge duplicate Java/Kotlin entries in same tables
- [ ] Change table type to `(&[&str], &[&str], &[&str])` (list of language names as first element)
- [ ] Update `scan_source_*` lookup logic to `languages.contains(&lang)`

## Phase 3: Generic Source Scanning + Extract Helpers
**Risk: Low | Snapshot updates: 0**

- [ ] Create `crates/detect/src/source_scanning.rs` with generic `scan_source_files()`
- [ ] Move `PORT_PATTERNS`, `HEALTH_PATTERNS`, `ENV_VAR_PATTERNS`, `BUILTIN_ENV_VARS` there
- [ ] Re-export `scan_source_ports()`, `scan_source_health()`, `scan_source_env_vars()` as wrappers
- [ ] Move `extract_project_dir()`, `replace_package()`, `extract_major_version()`, `extract_major_minor_version()` to `helpers.rs`
- [ ] Add `pub mod source_scanning;` to `lib.rs`

## Phase 4: Extract Version Readers
**Risk: Medium-Low | Snapshot updates: 0**

- [ ] Create `version/ruby.rs` — `read_ruby_version()`, `parse_gemfile_ruby_version()`, `parse_gemfile_lock_ruby_version()`
- [ ] Create `version/python.rs` — `read_python_version()`, `parse_pipfile_python_version()`
- [ ] Create `version/php.rs` — `read_php_version()`
- [ ] Create `version/swift.rs` — `read_swift_version()`
- [ ] Create `version/mise.rs` — `scan_mise_config()`, `read_mise_tools()`, `parse_mise_toml()`, `parse_tool_versions()`
- [ ] Extend `version/node.rs` with `read_node_version()`, `parse_node_version_string()`
- [ ] Update `version/mod.rs` to declare + re-export all submodules

## Phase 5: Extract Language-Specific Post-Processing
**Risk: Medium | Snapshot updates: 0**

- [ ] Create `postprocess/python.rs` — `scan_python_entrypoints()`, `fix_django_settings()`, `scan_python_native_deps()`, `collect_python_dep_names()`, `fix_flask_app_path()`, `dep_matches()`
- [ ] Create `postprocess/node.rs` — `scan_node_native_deps()`, `scan_node_system_deps()`, `scan_node_puppeteer()`, `sanitize_node_build_commands()`
- [ ] Create `postprocess/framework.rs` — `provide_framework_fallback_entrypoint()`, `wrap_yarn_corepack_entrypoint()`
- [ ] Move related tests to new modules
- [ ] Result: `pipeline.rs` drops from ~4269 to ~2200 lines

## Phase 6: Deduplicate Flask Detectors + Fix FlaskUvDetector
**Risk: Low-Medium | Snapshot updates: 0**

- [ ] Extract shared `flask_venv_contribution()` from identical `FlaskPoetryDetector` and `FlaskPdmDetector`
- [ ] Add doc comments explaining `detect()` returning `false` is by design (selected via `preferred_framework_env_keys`)

## Phase 7: Fix Connection/Retry Code Quality
**Risk: Medium | Snapshot updates: 0**

- [ ] Create `crates/buildkit/src/retry.rs` with generic `retry_with_backoff()`, use in `docker.rs` + `connection.rs`
- [ ] Extract `configure_endpoint()` in `connection.rs` (4 identical HTTP/2 configs)
- [ ] Normalize `keep_alive_timeout` across all connection variants (currently inconsistent: 3600s/600s/missing)
- [ ] Replace hand-rolled TOML parsing in `nixpacks_config.rs` with `toml` crate
- [ ] Document cache sharing mode (`Shared` is correct) in `builder.rs`

## Phase 8: Rework Swift Build to Use Wolfi (No Custom Docker Image)
**Risk: High | Snapshot updates: YES**

Swift is the ONLY language using `build_image` to bypass Wolfi. Swift is NOT in Wolfi apk packages.

- [ ] Add `setup_commands: Vec<String>` to `BuildStage` in schema — runs on Wolfi AFTER `apk add`, BEFORE build context mount
- [ ] Update `PeelboxStrategy` in `strategy.rs` to execute `setup_commands` step, remove `build_image` branch
- [ ] Update `PackageSwiftParser` — remove `build_image`, add `setup_commands` to download+install Swift toolchain tarball onto Wolfi
- [ ] Update `scan_version_files` for Swift — override setup_command URL version instead of `build_image`
- [ ] Remove `build_image` field from `BuildStage` schema entirely
- [ ] Update all Swift snapshot fixtures

## Phase 9: Fix Build Command Hacks & Health Endpoints
**Risk: Highest | Snapshot updates: YES**

- [ ] **Ruby sed hack**: Remove `sed -i '/^ruby /d' Gemfile` from `gemfile.rs` — stop deleting Gemfile ruby constraint, rely on `read_ruby_version()` to install correct version
- [ ] **Python venv sed hack**: Remove `find .venv/bin -exec sed ...` from `pyproject_toml.rs` — create venv at `/app/.venv` so shebangs are correct from the start
- [ ] **Health 404**: Remove `404 => return Ok(true)` from `container_harness.rs` — only 2xx is valid health
- [ ] **Health fallback**: In `e2e.rs`, try `/` as last-resort when no health endpoint detected
- [ ] **Health consistency**: Ensure all web framework detectors provide at least one health endpoint (fix FastAPI `vec![]`)
- [ ] **Node.js `n` docs**: Document runtime Node install limitation for old versions (<16) in `version/node.rs`
- [ ] Update affected snapshot fixtures

---

## Execution Notes

- Each phase = 1 commit
- Run `cargo fmt && cargo clippy --workspace` after each phase
- Phases 1–7: no snapshot changes
- Phases 8–9: regenerate snapshots with `UPDATE_SNAPSHOTS=1`
- Pipeline.rs reduction: ~4269 → ~2200 lines (48% smaller)

// TODO later:
source_scanning - should be merged with respective static data for languages