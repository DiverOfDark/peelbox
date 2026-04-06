# peelbox-detect

Map-reduce detection pipeline that scans repositories, parses manifests/configs, detects frameworks, partitions into services, and reduces to `UniversalBuild` specs. Uses inventory-based auto-registration for parsers and detectors.

## Module Structure

```
src/
├── lib.rs              # Public API: detect(), detect_with_registry(), detect_without_wolfi(), etc.
├── types.rs            # RepoTree, DirNode, TypedFile, FileKind, Manifest, BuildSpec, RuntimeSpec,
│                       # Workspace, MemberBuildTransform, ServiceBucket, ConfigContribution,
│                       # FrameworkContribution, Dependency, Package
├── traits.rs           # ManifestParser, ConfigParser, FrameworkDetector traits
├── registry.rs         # inventory-based auto-registration (ManifestParserEntry, ConfigParserEntry,
│                       # FrameworkDetectorEntry), Registry::with_defaults()
├── helpers.rs          # btree() helper
├── pipeline.rs         # Full detection pipeline (~1,725 lines) with Wolfi resolution + post-processing
├── ids.rs              # &'static str newtypes: LanguageId, BuildSystemId, FrameworkId, RuntimeId, OrchestratorId
│                       # Constants defined in parser/detector files, metadata via inventory
├── parsers/
│   ├── manifest/       # 40+ parsers (one file each, self-register via inventory::submit!)
│   │   │               # Version detection & Wolfi resolution colocated in primary parser per language:
│   │   ├── pom_xml     # Maven parser + shared Java version utils (detect/normalize/Wolfi resolution)
│   │   ├── build_gradle # Gradle parser + parse_gradle_version()
│   │   ├── package_json # npm/yarn/pnpm/bun parser + Node version/Wolfi + native deps + framework entrypoints
│   │   ├── cargo_toml  # Cargo parser + Rust version detection + Wolfi resolution
│   │   ├── pyproject_toml # Python parser + Python version + native deps + entrypoints + Django/Flask fixes
│   │   ├── gemfile     # Ruby parser + Ruby version detection
│   │   ├── composer_json # PHP parser + PHP version detection
│   │   ├── package_swift # Swift parser + Swift version detection
│   │   ├── ...         # Other parsers (go_mod, csproj, etc.)
│   │   └── mod.rs      # Shared parse_npm_deps()
│   └── config/         # 6 parsers
│       ├── env_file, dockerfile, docker_compose, kubernetes, app_config, procfile
│       ├── mise        # Mise/asdf tool version manager config scanning
│       └── mod.rs
└── framework_detectors/ # 10 files, 22+ detectors
    ├── mod.rs           # simple_detector! macro
    ├── nodejs.rs        # Express, Next.js, NestJs, Fastify
    ├── python.rs        # Django, Flask, FastAPI, Gunicorn, etc.
    ├── jvm.rs           # Spring Boot, Quarkus, Micronaut, Ktor
    ├── ruby.rs          # Rails, Sinatra, Puma, etc.
    ├── go_fw.rs         # Gin, Echo, etc.
    ├── rust_fw.rs       # Actix, Axum, Rocket, Warp, etc.
    ├── php.rs           # Laravel, Symfony, etc.
    ├── dotnet.rs        # ASP.NET, etc.
    ├── elixir.rs        # Phoenix
    └── zig_fw.rs        # Zig frameworks
```

## Pipeline (4 Stages + Post-processing)

1. **Parse** -- Walk repo, classify files into `TypedFile` (Manifest/Config/Source/Other)
2. **Framework Detect** -- Second pass over `Manifest.dependencies` to find frameworks
3. **Partition** -- Group manifests + configs into `ServiceBucket` by workspace topology
4. **Reduce** -- Convert each `ServiceBucket` into a `UniversalBuild`

Post-processing: Wolfi package resolution, source code scanning (health endpoints, env vars, version files, Python entrypoints).

## Inventory-Based Registration

Parsers and detectors self-register with zero central configuration:

```rust
// At bottom of each parser file:
inventory::submit! {
    crate::registry::ManifestParserEntry(|| Box::new(MyParser))
}
```

`Registry::with_defaults()` collects all registered implementations at runtime.

## Adding a New Manifest Parser

1. Create `src/parsers/manifest/my_format.rs`
2. Implement `ManifestParser` trait (`filenames()` + `parse()`)
3. Add `inventory::submit!` at bottom of file
4. Add `mod my_format; pub use my_format::MyFormatParser;` to `mod.rs`

That's it -- no registry updates needed.

## Adding a New Framework Detector

**Simple** (use macro):
```rust
simple_detector!(MyDetector, FrameworkId::MyFramework, &[LanguageId::Rust],
    |deps| deps.iter().any(|d| d.name == "my-lib"),
    vec![8080], vec!["/health".into()], BTreeMap::new(), vec![]);
inventory::submit! { crate::registry::FrameworkDetectorEntry(|| Box::new(MyDetector)) }
```

**Complex** (full trait implementation): Implement `FrameworkDetector` trait directly.

## Key Data Types

```
Manifest                          # Normalized from any format
├── language: LanguageId
├── build_system: BuildSystemId
├── runtime: RuntimeId
├── package: Option<Package>      # { name, version, is_application }
├── workspace: Option<Workspace>  # { members (globs), orchestrator }
├── dependencies: Vec<Dependency>
├── build: BuildSpec              # { packages, commands, member_transform, env, cache_dirs, artifacts }
└── runtime_config: RuntimeSpec   # { packages, env, entrypoint, workdir, ports, health_endpoint }

MemberBuildTransform              # Workspace-aware build commands
├── member_commands: Vec<String>  # With {module} and {root} template placeholders
└── member_artifacts: Option<Vec<(from, to)>>

ServiceBucket                     # Groups manifests + configs per service
├── manifest: Manifest
├── configs: Vec<ConfigContribution>
├── framework: Option<FrameworkContribution>
├── is_workspace_member: bool
└── workspace_root: Option<PathBuf>
```

## Important Gotchas

1. **`.csproj` files use extension-based matching**, not filename -- special-cased in `pipeline.rs`
2. **Lock files take priority** over `package.json` when both exist (prevents double-parsing)
3. **`FrameworkContribution` has 9 fields** -- easy to miss `runtime_command`, `runtime_env`, `workdir`, `extra_copy` in tests
4. **YarnLockParser/PnpmLockParser** return `None` for relative paths -- only work with absolute paths
5. **Python version constraints** (`^3.9`, `>=3.10`) are NOT valid for packages -- only exact versions from `.python-version`
6. **Node version from `.nvmrc`** overrides Wolfi-resolved version
7. **Gradle project name**: only from `rootProject.name` or `archivesBaseName` in content, NOT directory name
8. **Java version propagation**: workspace roots propagate versioned packages to members
9. **Poetry + Flask**: `FlaskPoetryDetector` overrides standard Flask contribution (sets `VIRTUAL_ENV`)
10. **Source scanning**: regex-based, only ports 1024-65535, case-sensitive health patterns, filters built-in env vars
11. **Subdirectory projects**: Maven uses `-f`, Cargo uses `--manifest-path`, others prepend `cd {dir} &&`
12. **Parsers return `Option<Manifest>`** -- `None` means "not my format" (not an error)

## Tests

57 tests. Run with: `cargo test -p peelbox-detect`

Test patterns: `tempfile::tempdir()` for file-based testing, verify both positive detection and false-positive prevention.
