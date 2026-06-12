# peelbox-core

Foundational crate providing the shared filesystem abstraction and the
`UniversalBuild` output schema. Every other crate in the workspace depends on this.

## Module Structure

```
src/
├── lib.rs           # Re-exports: FileSystem, MockFileSystem, RealFileSystem, UniversalBuild
├── fs/
│   ├── trait.rs     # FileSystem trait + FileType, FileMetadata, DirEntry
│   ├── real.rs      # RealFileSystem (std::fs wrapper)
│   └── mock.rs      # MockFileSystem (in-memory HashMap + RwLock)
└── output/
    └── schema.rs    # UniversalBuild, BuildMetadata, BuildStage, RuntimeStage, CopySpec, HealthCheck
```

## Key Types

### FileSystem Trait

Abstraction for filesystem operations. All implementations must be `Send + Sync`.

- `RealFileSystem` -- wraps `std::fs`
- `MockFileSystem` -- in-memory `HashMap<PathBuf, MockEntry>` with `RwLock`, defaults to `/mock` root. Auto-creates parent directories when adding files.

### UniversalBuild

The core output contract that all detection converges to:

```
UniversalBuild
├── version: String (default "1.0")
├── metadata: BuildMetadata { project_name, language, build_system, framework, reasoning }
├── build: BuildStage { packages, env (BTreeMap), commands, cache }
└── runtime: RuntimeStage { packages, env (BTreeMap), copy, command, workdir, ports, health }
```

- `BTreeMap` for env vars ensures deterministic serialization order
- Null/missing values in JSON/YAML default to empty (not error)
- `skip_serializing_if = "Option::is_none"` on optional fields

## Conventions

- Use `&dyn FileSystem` for testability -- inject the trait, not concrete types
- All string fields default to empty string, not null

## Tests

Run with: `cargo test -p peelbox-core`
