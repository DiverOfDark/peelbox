# framework-detection Specification Changes

## ADDED Requirements

### Requirement: Framework Trait Methods
The Framework trait SHALL provide methods for framework-specific runtime configuration.

#### Scenario: Framework runtime environment variables
- **WHEN** a framework is detected
- **THEN** the framework implementation provides `runtime_env_vars(service_path, port)` method
- **AND** returns framework-specific environment variables (e.g., ASP.NET Core returns `ASPNETCORE_URLS`)
- **AND** Runtime implementations do NOT contain framework-specific env var logic

#### Scenario: Framework entrypoint command
- **WHEN** a framework is detected
- **THEN** the framework implementation provides `entrypoint_command(files, port)` method
- **AND** returns framework-specific startup command (e.g., Flask returns `python -m flask run`)
- **AND** Runtime implementations do NOT contain framework-specific entrypoint logic

#### Scenario: Context-aware health endpoints
- **WHEN** a framework is detected
- **THEN** the framework implementation provides `health_endpoints(files)` method
- **AND** returns health endpoints based on project configuration
- **AND** Spring Boot checks for actuator dependency before returning `/actuator/health`
- **AND** returns empty vector if health endpoint unavailable

---

### Requirement: Separation of Concerns - Runtime vs Framework
Framework-specific logic SHALL reside in Framework implementations, not Runtime or Pipeline phases.

#### Scenario: No framework logic in Runtime
- **WHEN** implementing Runtime trait methods
- **THEN** the implementation contains ONLY language-level behavior
- **AND** does NOT check for specific frameworks (no `if framework == SpringBoot`)
- **AND** does NOT contain framework-specific commands or configurations

#### Scenario: No framework logic in Pipeline
- **WHEN** implementing Pipeline phase logic
- **THEN** the phase delegates to Framework trait methods for framework-specific behavior
- **AND** does NOT hardcode framework names or checks
- **AND** calls `framework.runtime_env_vars()` instead of hardcoding ASP.NET/Flask logic

#### Scenario: Framework-specific health endpoints
- **WHEN** determining health endpoints for a service
- **THEN** the system calls `framework.health_endpoints(files)`
- **AND** SpringBootFramework checks for actuator dependency in files
- **AND** FlaskFramework returns default Flask health endpoint
- **AND** Runtime implementations do NOT contain health endpoint logic

---

### Requirement: PHP Framework-Specific Extensions
PHP runtime SHALL detect and install framework-specific extensions based on composer.json dependencies.

#### Scenario: Laravel-specific extensions
- **WHEN** Laravel framework is detected in composer.json
- **THEN** the system installs `pdo_mysql`, `pdo_pgsql`, `redis` extensions if dependencies require them
- **AND** checks composer.json for database/cache dependencies

#### Scenario: Symfony-specific extensions
- **WHEN** Symfony framework is detected in composer.json
- **THEN** the system installs `intl`, `pdo_mysql`, `pdo_pgsql` extensions if dependencies require them
- **AND** checks composer.json for database dependencies

#### Scenario: WordPress-specific extensions
- **WHEN** WordPress is detected
- **THEN** the system installs `mysqli`, `gd` or `imagick`, `exif` extensions
- **AND** includes common WordPress extension requirements

#### Scenario: Common PHP extensions
- **WHEN** any PHP runtime is detected
- **THEN** the system installs essential extensions: `ctype`, `phar`, `openssl`, `mbstring`, `xml`, `dom`
- **AND** installs commonly needed extensions: `curl`, `json`, `session`, `tokenizer`, `fileinfo`, `iconv`

---

## MODIFIED Requirements

### Requirement: Generic Java Runtime Commands
Java runtime commands SHALL use generic `java` binary, not hardcoded JVM paths.

#### Scenario: Generic java command
- **WHEN** generating Java runtime startup command
- **THEN** the command uses `java -jar <jarfile>` without full path
- **AND** does NOT hardcode `/usr/lib/jvm/java-17-openjdk/bin/java`
- **AND** relies on PATH environment variable to resolve correct JVM version

---

## REMOVED Requirements

None - This change only adds new requirements and modifies existing implementation patterns.
