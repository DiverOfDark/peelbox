# Language Registry TODOs

## Language Consolidation

### Merge Kotlin into Java
- Kotlin should be a variant/flavor of JavaLanguage rather than a separate language
- Both use Gradle (and Maven via kotlin-maven-plugin)
- Detection: check for kotlin plugin in build.gradle(.kts)
- Single `JavaLanguage` with `KotlinFlavor` detection

### Merge TypeScript into JavaScript
- TypeScript is a superset of JavaScript, not a separate language
- Both use npm/yarn/pnpm/bun
- Detection: check for tsconfig.json or typescript in dependencies
- Single `JavaScriptLanguage` with TypeScript detection flag

## Runtime Version Detection

Languages should detect and recommend appropriate runtime versions based on project configuration:

### Java/Kotlin
- Detect Java version from:
  - `pom.xml`: `<maven.compiler.source>`, `<java.version>`, `<release>`
  - `build.gradle(.kts)`: `sourceCompatibility`, `targetCompatibility`, `toolchain`
  - `.java-version` file
  - `JAVA_HOME` indicator files
- Support versions: 8, 11, 17, 21 (LTS), 22, 23 (current)
- Default to latest LTS (21) if not specified

### Node.js/JavaScript/TypeScript
- Detect Node version from:
  - `.nvmrc`, `.node-version`
  - `package.json` `engines.node` field
  - `volta.node` in package.json
- Support versions: 18, 20, 22 (LTS), current
- Default to latest LTS if not specified

### Python
- Detect Python version from:
  - `pyproject.toml` `requires-python`
  - `Pipfile` `python_version`
  - `.python-version` file
  - `runtime.txt` (Heroku style)
- Support versions: 3.9, 3.10, 3.11, 3.12, 3.13
- Default to 3.11 if not specified

### Go
- Detect Go version from:
  - `go.mod` `go` directive
  - `.go-version` file
- Support versions: 1.21, 1.22, 1.23
- Default to version in go.mod

### Ruby
- Detect Ruby version from:
  - `.ruby-version` file
  - `Gemfile` ruby version constraint
  - `.tool-versions` (asdf)
- Support versions: 3.1, 3.2, 3.3
- Default to 3.2 if not specified

### .NET
- Detect .NET version from:
  - `*.csproj` `<TargetFramework>`
  - `global.json` sdk version
- Support versions: net6.0, net7.0, net8.0
- Default to net8.0 if not specified

## Move Scanner Constants to Language Definitions

The bootstrap scanner has hardcoded constants that should be moved to the language registry:

### Excluded Directories (`scanner.rs:is_excluded`)
Currently hardcoded in `BootstrapScanner::is_excluded()`:
- `.git`, `node_modules`, `target`, `dist`, `build`, `out`, `.next`, `.nuxt`
- `venv`, `.venv`, `__pycache__`, `.pytest_cache`, `vendor`
- `.idea`, `.vscode`, `coverage`, `.gradle`, `.m2`, `.cargo`

Should be:
- Add `excluded_dirs()` method to `LanguageDefinition` trait
- Each language provides its own excluded directories
- Scanner aggregates from all registered languages

### Workspace Configurations (`scanner.rs:is_workspace_config`)
Currently hardcoded in `BootstrapScanner::is_workspace_config()`:
- `pnpm-workspace.yaml`, `lerna.json`, `nx.json`, `turbo.json`, `rush.json`

Should be:
- Add `workspace_configs()` method to `LanguageDefinition` trait
- JavaScript/TypeScript language provides these workspace config files
- Other languages can provide their own (e.g., Cargo workspace, Gradle multi-project)

## Implementation Plan

1. Add `detect_version()` method to `LanguageDefinition` trait
2. Update `BuildTemplate` to accept version parameter
3. Implement version detection for each language
4. Update best practices templates to use detected version
5. Add tests for version detection edge cases
6. Add `excluded_dirs()` and `workspace_configs()` to `LanguageDefinition` trait
7. Migrate hardcoded constants from `BootstrapScanner` to language definitions
