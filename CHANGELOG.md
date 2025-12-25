# Changelog

All notable changes to aipack will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Wolfi-First Architecture**: All images use Wolfi packages exclusively
- **BuildKit Frontend**: Native LLB generation for optimal build performance
- **Distroless Images**: Mandatory 2-layer distroless final images (~10-30MB)
- **Context Transfer Optimization**: 99.995% reduction (1.5GB → ~100KB) via gitignore-based filtering
- **Dynamic Version Discovery**: Automatically detects available Wolfi package versions from APKINDEX
- **Package Validation**: Fuzzy matching and version-aware validation against Wolfi APKINDEX
- **Embedded LLM Backend**: Zero-config local inference with Qwen2.5-Coder (GGUF format)
- **Multi-Provider LLM Support**: Ollama, Claude, OpenAI, Gemini, Groq, xAI, embedded
- **Detection Modes**: Full (static + LLM), static-only, LLM-only modes
- **Binary APKINDEX Cache**: 30x performance improvement (70ms with warm cache)
- **SBOM and Provenance Support**: Documentation for BuildKit attestations
- **Integration Tests**: Comprehensive BuildKit integration test suite
- **13 Languages**: Rust, Java, Kotlin, JavaScript, TypeScript, Python, Go, C#, Ruby, PHP, C++, Elixir, F#
- **16 Build Systems**: Cargo, Maven, Gradle, npm, yarn, pnpm, Bun, pip, poetry, go mod, dotnet, composer, bundler, CMake, mix, pipenv
- **20 Frameworks**: Spring Boot, Quarkus, Next.js, Django, Rails, Actix-web, and more

### Changed

- **Schema Simplification**: Removed base image fields from UniversalBuild (version stays 1.0)
- **BuildSystem Trait**: Updated signature to accept `WolfiPackageIndex` and `manifest_content`
- **All BuildSystem Implementations**: Now query WolfiPackageIndex for dynamic version discovery
- **BuildTemplate Struct**: Removed `build_image` and `runtime_image` fields
- **Validation System**: Enhanced with Wolfi package validation and fuzzy matching
- **LLM Prompt Engineering**: Updated to request Wolfi packages instead of base images

### Removed

- **Dockerfile Generation**: Removed entirely in favor of BuildKit LLB
- **Base Image Configuration**: No longer supported (always uses Wolfi)
- **Traditional Base Images**: Debian, Ubuntu, Alpine no longer used
- **`build.base` Field**: Removed from UniversalBuild schema
- **`runtime.base` Field**: Removed from UniversalBuild schema
- **`build_image` Field**: Removed from BuildTemplate struct
- **`runtime_image` Field**: Removed from BuildTemplate struct

### Breaking Changes

**Schema Breaking Changes:**
- `UniversalBuild.build.base` field removed (schema version remains 1.0)
- `UniversalBuild.runtime.base` field removed (schema version remains 1.0)
- Base image is now hardcoded to `cgr.dev/chainguard/wolfi-base` (not configurable)

**API Breaking Changes:**
- `BuildSystem::build_template()` signature changed:
  - Old: `fn build_template(&self) -> BuildTemplate`
  - New: `fn build_template(&self, wolfi_index: &WolfiPackageIndex, manifest_content: Option<&str>) -> BuildTemplate`
- `BuildTemplate` struct fields changed:
  - Removed: `build_image: String`
  - Removed: `runtime_image: String`
  - Kept: `build_packages: Vec<String>`
  - Kept: `runtime_packages: Vec<String>`

**CLI Breaking Changes:**
- Dockerfile generation removed (no replacement - use BuildKit frontend instead)
- All output is now Wolfi-only (no traditional base image support)

**Behavior Breaking Changes:**
- All builds produce distroless final images (mandatory, no opt-out)
- Package names must be version-specific (e.g., `nodejs-22`, not `nodejs`)
- Package validation enforced (invalid packages cause errors)

### Migration Guide

**For Users:**

1. **Update Detection Output**:
   - Old output had `build.base` and `runtime.base` fields
   - New output only has `build.packages` and `runtime.packages` (Wolfi packages)
   - Schema version remains `1.0`

2. **Use BuildKit Frontend**:
   ```bash
   # Old: aipack detect . > Dockerfile
   # New:
   aipack detect . > universalbuild.json
   aipack frontend --spec universalbuild.json | buildctl build --local context=. --output type=docker,name=myapp:latest
   ```

3. **Package Names**:
   - Old: Generic package names (e.g., `nodejs`, `python`)
   - New: Version-specific Wolfi packages (e.g., `nodejs-22`, `python-3.12`)

4. **Final Images**:
   - All images are now distroless (no shell, no package manager)
   - Expected behavior: Smaller images (~10-30MB vs ~50-100MB)
   - Debugging: Use `docker exec` with debug sidecar or local dev tools

**For Library Users:**

1. **Update BuildSystem Implementations**:
   ```rust
   // Old:
   fn build_template(&self) -> BuildTemplate {
       BuildTemplate {
           build_image: "rust:1.75".to_string(),
           runtime_image: "debian:bookworm-slim".to_string(),
           build_packages: vec!["build-base".to_string()],
           runtime_packages: vec![],
       }
   }

   // New:
   fn build_template(&self, wolfi_index: &WolfiPackageIndex, manifest_content: Option<&str>) -> BuildTemplate {
       let rust_version = wolfi_index.get_latest_version("rust").unwrap_or("rust");
       BuildTemplate {
           build_packages: vec![rust_version, "build-base".to_string()],
           runtime_packages: vec![],
       }
   }
   ```

2. **Remove Base Image Handling**:
   - Delete any code that configures or selects base images
   - Update tests to expect Wolfi packages instead of base images

3. **Validate Against Wolfi**:
   - Use `Validator::with_wolfi_index()` to validate packages
   - Expect version-specific package names in all outputs

### Security

- **Reduced Attack Surface**: Distroless images eliminate shells, package managers, and unnecessary tools
- **Daily Security Updates**: Wolfi packages receive daily security patches from Chainguard
- **SBOM Support**: Built-in SBOM generation via BuildKit for vulnerability tracking
- **Provenance Support**: SLSA provenance attestations for supply chain security
- **No Legacy Vulnerabilities**: Wolfi packages built from source without legacy CVEs

### Performance

- **Context Transfer**: 99.995% reduction (1.5GB → 80-113KB) via gitignore filtering
- **APKINDEX Caching**: Binary cache provides 30x speedup (70ms with warm cache)
- **Image Size**: 50-90% smaller than traditional base images
- **Build Speed**: Optimized 2-layer caching (runtime base + app separate)

### Technical Debt

The following technical debt items are tracked but not yet implemented:

- TD-1: Add .nvmrc/.node-version support for Node.js version detection
- TD-2: Add runtime version detection from manifest files (PHP, Python, Ruby, Go)
- TD-3: Fix Gradle detection to prefer build.gradle.kts over settings.gradle.kts
- TD-4: Verify RuntimeTrait packages are used in final output
- TD-5: Add LLM backend support for deno-fresh and zig-build test fixtures
- TD-6: Review and cleanup wolfi_index.rs implementation
- TD-7: Move runtime_packages from BuildSystemTrait to RuntimeTrait

## [0.1.0] - Initial Development

### Added

- Initial 9-phase pipeline architecture
- LLM-powered build system detection
- Support for 13 programming languages
- Support for 16 build systems
- Support for 20 frameworks
- Static analysis with LLM fallback
- JSON/YAML output formats
- Confidence scoring and reasoning
- Mock filesystem for testing
- Comprehensive test suite

[Unreleased]: https://github.com/diverofdark/aipack/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/diverofdark/aipack/releases/tag/v0.1.0
