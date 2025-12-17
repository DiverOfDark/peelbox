# Spec Delta: prompt-pipeline

## ADDED Requirements

### Requirement: Framework Detection from Dependencies

The system SHALL detect frameworks by matching dependency patterns against FrameworkRegistry. Framework detection MUST be deterministic for major frameworks (confidence >= 0.9) and MUST NOT require LLM calls when dependency patterns match.

#### Scenario: Spring Boot detection from Maven pom.xml

**Given** a Java project with Maven build system
**When** dependencies include `org.springframework.boot:spring-boot-starter-web`
**Then** framework is detected as "Spring Boot"
**And** framework confidence is 0.95 (High)
**And** framework is compatible with Java language
**And** framework is compatible with Maven build system

#### Scenario: Express detection from npm package.json

**Given** a JavaScript project with npm build system
**When** dependencies include `express` package
**Then** framework is detected as "Express"
**And** framework confidence is 0.95 (High)
**And** framework is compatible with JavaScript language
**And** framework is compatible with npm/yarn/pnpm build systems

#### Scenario: Multiple framework candidates

**Given** a Python project with both `django` and `flask` packages
**When** dependencies are analyzed
**Then** framework with highest confidence is selected
**And** if confidence scores are equal, first match is used
**And** warning is logged about multiple framework candidates

#### Scenario: Framework compatibility validation

**Given** a detected framework and language/build system combination
**When** framework compatibility is checked
**Then** FrameworkRegistry validates the combination
**And** invalid combinations are rejected (e.g., Spring Boot + Python)
**And** error is logged if combination is invalid

### Requirement: Language-Framework-BuildSystem Relationships

The system SHALL maintain explicit many-to-many relationships between languages, frameworks, and build systems. Each framework MUST declare compatible languages and build systems. The system MUST validate framework compatibility before accepting detection results.

#### Scenario: Spring Boot compatibility declaration

**Given** Spring Boot framework definition
**When** querying compatible languages
**Then** returns ["Java", "Kotlin"]
**When** querying compatible build systems
**Then** returns ["maven", "gradle"]

#### Scenario: Next.js compatibility declaration

**Given** Next.js framework definition
**When** querying compatible languages
**Then** returns ["JavaScript", "TypeScript"]
**When** querying compatible build systems
**Then** returns ["npm", "yarn", "pnpm", "bun"]

#### Scenario: Relationship validation

**Given** a project with Java language, Maven build system, Spring Boot framework
**When** relationship validation is performed
**Then** combination is valid
**And** detection proceeds normally

**Given** a project with Python language, pip build system, Spring Boot framework
**When** relationship validation is performed
**Then** combination is invalid
**And** framework detection is rejected
**And** error message indicates incompatibility

### Requirement: Framework-Specific Build Customization

Frameworks SHALL customize build templates with framework-specific optimizations. The system MUST allow frameworks to modify artifact paths, build commands, and runtime commands via the `customize_build_template()` method.

#### Scenario: Spring Boot fat JAR artifact

**Given** a Spring Boot project with Maven
**When** build template is generated
**Then** framework customizes artifact path to `target/*.jar`
**And** build commands include Spring Boot Maven plugin
**And** runtime command uses `java -jar`

#### Scenario: Next.js build output structure

**Given** a Next.js project with npm
**When** build template is generated
**Then** framework customizes artifact path to `.next/` and `public/`
**And** build commands include `next build`
**And** runtime command uses `next start`

#### Scenario: No framework customization

**Given** a project with unknown or no framework
**When** build template is generated
**Then** default build system template is used
**And** no framework-specific customizations are applied
