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

## Implementation Plan

1. Add `detect_version()` method to `LanguageDefinition` trait
2. Update `BuildTemplate` to accept version parameter
3. Implement version detection for each language
4. Update best practices templates to use detected version
5. Add tests for version detection edge cases
