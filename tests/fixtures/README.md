# Test Fixtures

Comprehensive test fixtures for verifying aipack's build system detection across different languages, build tools, and project structures.

## Directory Structure

```
fixtures/
├── single-language/   # Single build system projects
├── monorepo/          # Monorepo/workspace projects
├── edge-cases/        # Edge cases and unusual configurations
└── expected/          # Expected JSON outputs (future)
```

## Single-Language Fixtures

### Rust
- **rust-cargo**: Standard Rust project with Cargo.toml, dependencies, and tests
- **rust-workspace**: Cargo workspace with multiple members (lib-a, lib-b, app)

### Node.js/TypeScript
- **node-npm**: TypeScript project with npm, Express, Jest
- **node-yarn**: Same as node-npm but with yarn.lock
- **node-pnpm**: Same as node-npm but with pnpm-lock.yaml

### Python
- **python-pip**: Flask app with requirements.txt and pytest
- **python-poetry**: Same app using Poetry (pyproject.toml)

### JVM Languages
- **java-maven**: Spring Boot app with Maven (pom.xml)
- **java-gradle**: Spring Boot app with Gradle (build.gradle)
- **kotlin-gradle**: Spring Boot app in Kotlin with Gradle Kotlin DSL

### Other Languages
- **go-mod**: Gin web server with go.mod
- **dotnet-csproj**: ASP.NET Core minimal API with .csproj

## Monorepo Fixtures

### JavaScript/TypeScript Monorepos
- **npm-workspaces**: npm workspaces with packages/* and apps/*
- **turborepo**: Turborepo configuration with turbo.json pipeline

### Cargo Workspace
- **cargo-workspace**: Rust workspace (same as rust-workspace)

### JVM Monorepos
- **gradle-multiproject**: Gradle multi-project with settings.gradle
- **maven-multimodule**: Maven multi-module with parent/child poms

### Mixed Language
- **polyglot**: Frontend (Node.js), Backend (Java), CLI (Rust)

## Edge Cases

- **empty-repo**: Completely empty repository (only README)
- **no-manifest**: Source code without build manifest
- **multiple-manifests**: Mixed build systems (Cargo + npm + Maven)
- **nested-projects**: Projects within projects (outer/inner)
- **vendor-heavy**: Project with large vendor directory

## Usage

These fixtures test that aipack can:

1. **Detect primary language and build system**
   - Identify Rust/Cargo, Node.js/npm, Python/pip, Java/Maven, etc.

2. **Handle workspaces/monorepos**
   - Detect workspace structure
   - Identify all subprojects
   - Generate appropriate workspace-level commands

3. **Handle edge cases gracefully**
   - Empty repositories
   - Missing manifests
   - Conflicting build systems
   - Nested project structures

## Verification

All fixtures are:
- ✅ **Minimal**: Only essential files for detection
- ✅ **Representative**: Real-world project structures
- ✅ **Working**: Can actually build/run with the specified tools
- ✅ **Complete**: Include source code, manifests, and dependencies

## Expected Outputs

Expected `UniversalBuild` JSON outputs for each fixture will be stored in `expected/` directory. These serve as golden files for regression testing.
