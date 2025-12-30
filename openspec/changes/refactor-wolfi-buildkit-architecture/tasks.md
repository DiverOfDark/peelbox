# Implementation Tasks

## 1. BuildKit LLB Improvements
- [x] Remove duplicate BusyBox image loads (llb.rs lines 211, 224)
- [x] Remove duplicate Wolfi base loads (llb.rs lines 86, 95)
- [x] Extract normalize_path() helper function (llb.rs lines 158-162, 238-242)
- [x] Extract is_directory() helper function (llb.rs lines 164-168, 244+259-260)
- [x] Separate build commands into individual layers using direct .run() calls (llb.rs line 151)
- [x] Keep artifact copy as final layer after build commands

## 2. Cache Directory Fixes
- [x] Fix Maven cache: `/tmp/maven-repo` → `/root/.m2` (buildsystem/maven.rs:78)
- [x] Fix Gradle cache: `/tmp/gradle-home` → `/root/.gradle` (buildsystem/gradle.rs:112)
- [x] Fix Poetry cache: `/tmp/poetry-cache` → `/root/.cache/pypoetry` (buildsystem/poetry.rs:75,88)
- [x] Fix Pipenv cache: `/tmp/pipenv-cache` → `/root/.cache/pipenv` (buildsystem/pipenv.rs:57,71)
- [x] Fix npm: Remove `HOME=/tmp`, use default `~/.npm` cache (buildsystem/npm.rs:81,85-86)
- [x] Fix pnpm: Remove `HOME=/tmp` (buildsystem/pnpm.rs:79)
- [x] Fix yarn: Remove `HOME=/tmp` (buildsystem/yarn.rs:79)
- [x] Fix dotnet: `/tmp` → `/root` (buildsystem/dotnet.rs:95)

## 3. Framework Trait Extensions
- [x] Add `runtime_env_vars()` method to Framework trait (framework/mod.rs)
- [x] Add `entrypoint_command()` method to Framework trait (framework/mod.rs)
- [x] Modify existing `health_endpoints()` signature to accept `files: &[PathBuf]` (framework/mod.rs)

## 4. Move Hardcoded Logic to Frameworks
- [x] AspNetFramework: Implement `runtime_env_vars()` for ASPNETCORE_URLS (framework/aspnet.rs)
- [x] FlaskFramework: Implement `runtime_env_vars()` and `entrypoint_command()` (framework/flask.rs)
- [x] DjangoFramework: Implement `entrypoint_command()` (framework/django.rs)
- [x] FastApiFramework: Implement `entrypoint_command()` (framework/fastapi.rs)
- [x] SpringBootFramework: Implement `health_endpoints(files)` with actuator detection (framework/springboot.rs)
- [x] Remove hardcoded ASP.NET/Flask logic from assemble.rs (phases/08_assemble.rs:174-201)
- [x] Remove `has_spring_boot_actuator` from jvm.rs (runtime/jvm.rs:93-113)
- [x] Remove Spring Boot health logic from jvm.rs (runtime/jvm.rs:133-150)
- [x] Fix hardcoded Java version in jvm.rs (runtime/jvm.rs:171)
- [x] Remove `detect_framework_entrypoint` from python.rs (runtime/python.rs:161-206)
- [x] Remove `find_flask_app` from python.rs (runtime/python.rs:208-232)

## 5. Schema Changes (BREAKING)
- [x] Remove `confidence` field from BuildMetadata (output/schema.rs:52)
- [x] Remove `artifacts` field from BuildStage (output/schema.rs:67-68)
- [x] Update LLB to extract artifacts from `runtime.copy[].from` (buildkit/llb.rs)
- [x] Update all test fixtures to remove confidence and artifacts fields
- [x] Update build system templates to only populate runtime.copy

## 6. PHP Extensions
- [x] Add curl, fileinfo, iconv to required_extensions (runtime/php.rs:125)
- [x] Implement `detect_framework_extensions()` for Laravel, Symfony, WordPress (runtime/php.rs)
- [x] Add framework-specific extensions based on composer.json dependencies (runtime/php.rs)

## 7. Monorepo Service Selection
- [x] Add `--service` CLI flag to FrontendCommand (cli/mod.rs)
- [x] Add `ensure_unique_service_names()` validation in detection (detection/service.rs)
- [x] Update `write_definition()` with strict service selection, no fallback (buildkit/llb.rs)
- [x] Update CLAUDE.md with monorepo usage examples (CLAUDE.md)

## 8. Test Fixtures
- [ ] Create static-html fixture with nginx (tests/fixtures/single-language/static-html/)
- [ ] Add e2e tests for static-html (detection + container) (tests/e2e.rs)
- [ ] Create dockerfile-exists fixture using docker/dockerfile:1 frontend (tests/fixtures/edge-cases/dockerfile-exists/)
- [ ] Add e2e tests for dockerfile-exists (detection + LLB generation) (tests/e2e.rs)
- [ ] Implement HTTP server for cpp-cmake (Crow/httplib) (tests/fixtures/single-language/cpp-cmake/)
- [ ] Implement HTTP server for elixir-mix (Plug) (tests/fixtures/single-language/elixir-mix/)
- [ ] Enforce health endpoint requirement in e2e.rs (tests/e2e.rs)

## 9. Java Build Handling
- [x] Gradle: Copy all JARs (`build/libs/*.jar`), let runtime determine executable (buildsystem/gradle.rs)
- [x] Maven: Add `dependency:copy-dependencies` goal (buildsystem/maven.rs)
- [x] Maven: Update runtime.copy to include `target/lib/` (buildsystem/maven.rs)
- [x] Maven: Update runtime.command to use classpath with dependencies (buildsystem/maven.rs)

## 10. Cleanup
- [x] Fix python-poetry duplicate copy (remove `.venv/` entry) (tests/fixtures/single-language/python-poetry/universalbuild.json)
- [x] Update root universalbuild.json to single service (aipack only) (universalbuild.json)
- [x] Simplify `build_services_from_workspace` with helper function (phases/07_service_analysis.rs:71-123)
- [x] Add separation of concerns rule to CLAUDE.md (CLAUDE.md)
