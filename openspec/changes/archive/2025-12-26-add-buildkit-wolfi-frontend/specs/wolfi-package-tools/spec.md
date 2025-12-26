## ADDED Requirements

### Requirement: Wolfi Package Validation Tool
The system SHALL provide an LLM tool to validate that package names exist in the Wolfi repository.

#### Scenario: Validate existing packages
- **WHEN** `validate_wolfi_packages` is called with package names `["rust", "ca-certificates"]`
- **THEN** the tool returns validation results indicating both packages exist
- **AND** includes package version information for each valid package

#### Scenario: Validate non-existent package
- **WHEN** `validate_wolfi_packages` is called with package name `["nonexistent-package"]`
- **THEN** the tool returns validation failure for that package
- **AND** suggests similar package names if any exist

#### Scenario: Validate mixed packages
- **WHEN** `validate_wolfi_packages` is called with `["nodejs-22", "libfoo-invalid"]`
- **THEN** the tool returns success for `nodejs-22` and failure for `libfoo-invalid`
- **AND** the LLM can use results to correct the package list

---

### Requirement: Wolfi Package Search Tool
The system SHALL provide an LLM tool to search for packages by keyword or description.

#### Scenario: Search by keyword
- **WHEN** `search_wolfi_packages` is called with query `"python"`
- **THEN** the tool returns packages matching the keyword (e.g., `python-3.12`, `py3-pip`, `py3-setuptools`)
- **AND** includes package descriptions for each result

#### Scenario: Search for build tools
- **WHEN** `search_wolfi_packages` is called with query `"compiler"`
- **THEN** the tool returns relevant packages (e.g., `gcc`, `clang`, `build-base`)
- **AND** results are sorted by relevance

#### Scenario: Search with no results
- **WHEN** `search_wolfi_packages` is called with query `"xyznonexistent"`
- **THEN** the tool returns an empty result set
- **AND** suggests broadening the search terms

---

### Requirement: Wolfi Package Index Caching
The system SHALL cache the Wolfi package index to avoid repeated network requests.

#### Scenario: Initial index fetch
- **WHEN** a package tool is called and no cache exists
- **THEN** the system fetches `APKINDEX.tar.gz` from `packages.wolfi.dev`
- **AND** parses and caches the package list locally

#### Scenario: Cache refresh
- **WHEN** the cached index is older than 24 hours
- **THEN** the system refreshes the cache on next tool invocation
- **AND** continues to serve requests from stale cache if refresh fails

#### Scenario: Offline operation
- **WHEN** network is unavailable and cache exists
- **THEN** the system uses the cached index
- **AND** logs a warning about potentially stale package data

---

### Requirement: Package Tool Integration with Detection
The system SHALL use package tools during build detection to ensure valid package names.

#### Scenario: Validate packages before submission
- **WHEN** the LLM calls `submit_detection` with package names
- **THEN** the system validates all package names against the Wolfi index
- **AND** returns an error if invalid packages are specified

#### Scenario: LLM searches for unknown package
- **WHEN** the LLM needs a package for an uncommon dependency
- **THEN** the LLM can call `search_wolfi_packages` to find the correct name
- **AND** uses the validated name in the build specification
