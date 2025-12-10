## 1. BuildKit Client Integration

- [ ] 1.1 Add `buildkit-client` dependency to `Cargo.toml`
- [ ] 1.2 Create `src/buildkit/mod.rs` module structure
- [ ] 1.3 Implement `BuildKitClient` wrapper with connection management
- [ ] 1.4 Support Unix socket connection (default)
- [ ] 1.5 Support TCP connection with optional TLS
- [ ] 1.6 Support Docker container connection (`docker-container://`)
- [ ] 1.7 Implement BuildKit version check (require v0.11.0+)
- [ ] 1.8 Add connection error handling with helpful messages
- [ ] 1.9 Add unit tests for connection parsing and version validation

## 2. LLB Graph Generation

- [ ] 2.1 Create `src/buildkit/llb.rs` module
- [ ] 2.2 Implement `LLBBuilder` struct that generates LLB from UniversalBuild
- [ ] 2.3 Generate source operation for wolfi-base image pull
- [ ] 2.4 Generate exec operations for apk package installation
- [ ] 2.5 Generate exec operations for build commands
- [ ] 2.6 Generate copy operations for build context
- [ ] 2.7 Generate copy operations for artifacts (build → runtime)
- [ ] 2.8 Implement cache mount generation with deterministic IDs
- [ ] 2.9 Implement multi-stage graph structure (build + runtime)
- [ ] 2.10 Add metadata labels to final image
- [ ] 2.11 Add unit tests for LLB generation

## 3. Wolfi Package Tools

*Note: After `restructure-ai-pipeline`, tools live in `src/tools/` and implement `Tool` trait.*

- [ ] 3.1 Create `src/tools/wolfi_packages.rs` module
- [ ] 3.2 Implement APKINDEX.tar.gz fetcher from `packages.wolfi.dev`
- [ ] 3.3 Implement APK index parser to extract package names and descriptions
- [ ] 3.4 Implement local caching with 24-hour TTL in `src/tools/wolfi_cache.rs`
- [ ] 3.5 Implement `ValidateWolfiPackagesTool` implementing `Tool` trait
- [ ] 3.6 Implement `SearchWolfiPackagesTool` implementing `Tool` trait
- [ ] 3.7 Register tools in `ToolRegistry`
- [ ] 3.8 Add unit tests for package validation and search (using `MockFileSystem`)

## 4. Wolfi Package Validation Integration

*Note: After `restructure-ai-pipeline`, validation uses `Validator` with `ValidationRule` trait.*

- [ ] 4.1 Create `WolfiPackageValidationRule` implementing `ValidationRule` trait
- [ ] 4.2 Validate build.packages against Wolfi index
- [ ] 4.3 Validate runtime.packages against Wolfi index
- [ ] 4.4 Return detailed error with suggestions for invalid packages
- [ ] 4.5 Register rule in `Validator`
- [ ] 4.6 Add unit tests for validation rule

## 5. Language Registry Wolfi Support

*Note: After `restructure-ai-pipeline`, best practices come from `LanguageRegistry`.*

- [ ] 5.1 Add `wolfi_build_packages()` method to `LanguageDefinition` trait
- [ ] 5.2 Add `wolfi_runtime_packages()` method to `LanguageDefinition` trait
- [ ] 5.3 Update `RustLanguage` with Wolfi packages (`rust`, `build-base`, `glibc`)
- [ ] 5.4 Update `JavaScriptLanguage` with Wolfi packages (`nodejs-22`)
- [ ] 5.5 Update `PythonLanguage` with Wolfi packages (`python-3.12`, `py3-pip`)
- [ ] 5.6 Update `GoLanguage` with Wolfi packages (`go`)
- [ ] 5.7 Update `JavaLanguage` with Wolfi packages (`openjdk-21`, `openjdk-21-jre`)
- [ ] 5.8 Update remaining languages with appropriate Wolfi packages
- [ ] 5.9 Add unit tests for Wolfi package retrieval

## 6. Bootstrap Context Wolfi Support

*Note: After `restructure-ai-pipeline`, system prompt is built by `BootstrapScanner`.*

- [ ] 6.1 Add `WolfiPackageHints` to `BootstrapContext`
- [ ] 6.2 Include detected language's Wolfi packages in bootstrap context
- [ ] 6.3 Update `format_for_prompt()` to include Wolfi guidance
- [ ] 6.4 Guide LLM to use `validate_wolfi_packages` and `search_wolfi_packages` tools
- [ ] 6.5 Add unit tests for bootstrap Wolfi context

## 7. SBOM Generation

- [ ] 7.1 Research BuildKit SBOM attestation API via `buildkit-client`
- [ ] 7.2 Implement SBOM attestation request in build solve options
- [ ] 7.3 Enable `BUILDKIT_SBOM_SCAN_CONTEXT` for full scanning
- [ ] 7.4 Configure Syft scanner options
- [ ] 7.5 Add `--no-sbom` flag to disable SBOM generation
- [ ] 7.6 Add tests for SBOM attachment to image manifest

## 8. Provenance Generation

- [ ] 8.1 Implement SLSA provenance attestation request
- [ ] 8.2 Include aipack version in provenance metadata
- [ ] 8.3 Include detected language and build system in provenance
- [ ] 8.4 Include git repository URL if available
- [ ] 8.5 Add `--no-provenance` flag to disable provenance
- [ ] 8.6 Add `--no-attestations` flag to disable all attestations

## 9. Build Command CLI

- [ ] 9.1 Add `build` subcommand to CLI in `src/cli/commands.rs`
- [ ] 9.2 Implement `--tag` flag for image naming
- [ ] 9.3 Implement `--push` flag for registry push
- [ ] 9.4 Implement `--output` flag for export type (docker, oci, tar)
- [ ] 9.5 Implement `--buildkit` flag for endpoint configuration
- [ ] 9.6 Support `BUILDKIT_HOST` environment variable
- [ ] 9.7 Implement `--quiet` and `--verbose` flags
- [ ] 9.8 Add help text and examples for build command

## 10. Build Progress Display

- [ ] 10.1 Create `src/buildkit/progress.rs` module
- [ ] 10.2 Implement progress event handler for BuildKit stream
- [ ] 10.3 Display layer build status (building, cached, done)
- [ ] 10.4 Display cache hit/miss information
- [ ] 10.5 Display final image digest on completion
- [ ] 10.6 Implement quiet mode (errors only)
- [ ] 10.7 Implement verbose mode (full logs)

## 11. Remove Dockerfile Generation

- [ ] 11.1 Remove `src/output/dockerfile.rs` module
- [ ] 11.2 Remove `--format dockerfile` from CLI (if present)
- [ ] 11.3 Remove `to_dockerfile()` method from UniversalBuild
- [ ] 11.4 Update documentation to reflect BuildKit-only workflow
- [ ] 11.5 Remove Dockerfile-related tests

## 12. Schema Simplification

- [ ] 12.1 Remove `package_manager` field from UniversalBuild (always apk)
- [ ] 12.2 Update schema documentation to specify Wolfi packages
- [ ] 12.3 Update schema validation for Wolfi-only workflow
- [ ] 12.4 Add migration notes for schema changes

## 13. Integration and Testing

- [ ] 13.1 Create integration test for full build workflow
- [ ] 13.2 Test build with local BuildKit daemon
- [ ] 13.3 Test SBOM and provenance attestation generation
- [ ] 13.4 Test various output types (docker, oci, tar)
- [ ] 13.5 Test error handling for missing BuildKit daemon
- [ ] 13.6 Test error handling for BuildKit version < 0.11.0
- [ ] 13.7 Test cache mount behavior across builds
- [ ] 13.8 Test Wolfi package validation and search tools
- [ ] 13.9 Add E2E test fixtures for Wolfi builds (after Phase 15 of restructure)

## 14. Documentation

- [ ] 14.1 Update README.md with new `build` command
- [ ] 14.2 Document BuildKit daemon setup requirements (v0.11.0+)
- [ ] 14.3 Document Docker Desktop 4.17+ / Docker Engine 23.0+ requirements
- [ ] 14.4 Document Wolfi package names for common languages
- [ ] 14.5 Add examples for CI/CD integration
- [ ] 14.6 Update CLAUDE.md with new architecture
- [ ] 14.7 Update CHANGELOG.md with breaking changes
