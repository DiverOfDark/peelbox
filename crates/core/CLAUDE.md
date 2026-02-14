# peelbox-core

Foundational crate providing shared configuration, error types, filesystem abstraction, and the `UniversalBuild` output schema. Every other crate in the workspace depends on this.

## Module Structure

```
src/
├── lib.rs           # Re-exports: PeelboxConfig, BackendError, FileSystem, MockFileSystem, RealFileSystem, UniversalBuild
├── config.rs        # Configuration management with env var support
├── error.rs         # BackendError enum for LLM failure modes
├── fs/
│   ├── trait.rs     # FileSystem trait + FileType, FileMetadata, DirEntry
│   ├── real.rs      # RealFileSystem (std::fs wrapper)
│   └── mock.rs      # MockFileSystem (in-memory HashMap + RwLock)
└── output/
    └── schema.rs    # UniversalBuild, BuildMetadata, BuildStage, RuntimeStage, CopySpec, HealthCheck
```

## Key Types

### PeelboxConfig

Centralized configuration loaded from environment variables with validation.

| Field | Env Var | Default | Range |
|-------|---------|---------|-------|
| `provider` | `PEELBOX_PROVIDER` | Ollama | ollama, openai, claude, gemini, grok, groq |
| `model` | `PEELBOX_MODEL` | qwen2.5-coder:7b | Provider-specific |
| `request_timeout_secs` | `PEELBOX_REQUEST_TIMEOUT` | 30 | 1-3600 |
| `max_context_size` | `PEELBOX_MAX_CONTEXT_SIZE` | 512000 | 1KB-10MB |
| `max_tokens` | `PEELBOX_MAX_TOKENS` | 8192 | 512-128000 |
| `cache_enabled` | `PEELBOX_CACHE_ENABLED` | true | |
| `cache_dir` | `PEELBOX_CACHE_DIR` | `{temp_dir}/peelbox-cache` | |

**Always call `config.validate()`** before use -- there is no automatic validation.

### DetectionMode

Controls pipeline behavior via `PEELBOX_DETECTION_MODE`:
- `Full` (default) -- static + LLM detection
- `StaticOnly` -- manifest/config parsing only
- `LLMOnly` -- LLM-based analysis only

### BackendError

Serializable error enum covering LLM failure modes: `ApiError`, `AuthenticationError`, `TimeoutError`, `RateLimitError`, `InvalidResponse`, `ConfigurationError`, `NetworkError`, `ParseError`, `Other`.

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
- Cache path sanitizes special characters (/, \, :, *, ?, ", <>, |) to underscores
- All string fields default to empty string, not null
- Environment variable tests use `serial_test` crate for isolation

## Tests

31 tests across 4 modules. Run with: `cargo test -p peelbox-core`
