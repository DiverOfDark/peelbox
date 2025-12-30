# build-systems Specification Changes

## ADDED Requirements

### Requirement: Persistent Cache Directories
Build systems SHALL use persistent cache directories that survive across BuildKit builds.

#### Scenario: Maven cache persistence
- **WHEN** Maven build system is detected
- **THEN** `MAVEN_OPTS` is set to `-Dmaven.repo.local=/root/.m2`
- **AND** cache path includes `/root/.m2/repository`
- **AND** cache persists across BuildKit cache mounts

#### Scenario: Gradle cache persistence
- **WHEN** Gradle build system is detected
- **THEN** `GRADLE_USER_HOME` is set to `/root/.gradle`
- **AND** cache path includes `/root/.gradle/caches`
- **AND** cache persists across BuildKit cache mounts

#### Scenario: Poetry cache persistence
- **WHEN** Poetry build system is detected
- **THEN** `POETRY_CACHE_DIR` is set to `/root/.cache/pypoetry`
- **AND** cache path includes `/root/.cache/pypoetry/`
- **AND** cache persists across BuildKit cache mounts

#### Scenario: Pipenv cache persistence
- **WHEN** Pipenv build system is detected
- **THEN** `PIPENV_CACHE_DIR` is set to `/root/.cache/pipenv`
- **AND** cache paths include `/root/.cache/pip/` and `/root/.cache/pipenv/`
- **AND** cache persists across BuildKit cache mounts

#### Scenario: npm cache persistence
- **WHEN** npm build system is detected
- **THEN** default `~/.npm` cache location is used
- **AND** `HOME` environment variable is NOT set to `/tmp`
- **AND** cache path includes `/root/.npm/`
- **AND** cache persists across BuildKit cache mounts

#### Scenario: pnpm cache persistence
- **WHEN** pnpm build system is detected
- **THEN** default pnpm cache location is used
- **AND** `HOME` environment variable is NOT set to `/tmp`
- **AND** cache persists across BuildKit cache mounts

#### Scenario: yarn cache persistence
- **WHEN** yarn build system is detected
- **THEN** default yarn cache location is used
- **AND** `HOME` environment variable is NOT set to `/tmp`
- **AND** cache persists across BuildKit cache mounts

#### Scenario: dotnet cache persistence
- **WHEN** dotnet build system is detected
- **THEN** `DOTNET_CLI_HOME` is set to `/root`
- **AND** cache persists across BuildKit cache mounts

---

### Requirement: No /tmp Cache Directories
Build systems SHALL NOT use `/tmp` for cache directories as it is ephemeral storage.

#### Scenario: Reject /tmp cache paths
- **WHEN** any build system is configured
- **THEN** cache directories MUST NOT be under `/tmp/`
- **AND** cache directories MUST use persistent paths under `/root/`
- **AND** BuildKit `Mount::SharedCache()` can properly cache these locations

---

## MODIFIED Requirements

### Requirement: Java JAR Artifact Handling
Build systems SHALL handle Java JAR artifacts without assuming user configuration.

#### Scenario: Gradle with multiple JARs
- **WHEN** Gradle build produces multiple JAR files
- **THEN** the system copies all JARs from `build/libs/*.jar`
- **AND** lets the JVM runtime determine which JAR is executable based on manifest
- **AND** does NOT assume `jar.enabled = false` is configured

#### Scenario: Maven with thin JARs
- **WHEN** Maven build is detected for Spring Boot
- **AND** `spring-boot-maven-plugin` repackage goal may not be configured
- **THEN** the system adds `dependency:copy-dependencies` goal
- **AND** copies both `target/*.jar` and `target/lib/` directory
- **AND** configures runtime command to use classpath with dependencies
