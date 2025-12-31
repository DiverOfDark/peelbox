# Design - Dual-Mode Testing

## Overview
This design adds dual-mode e2e testing by introducing environment variable control for detection mode. All tests spawn the CLI binary with different `PEELBOX_DETECTION_MODE` values to validate both LLM and static analysis paths.

## Test Structure

### Single Test File with Dual Variants

```
tests/e2e.rs
├── test_rust_cargo_llm()          # Spawns CLI with PEELBOX_DETECTION_MODE=llm
├── test_rust_cargo_static()       # Spawns CLI with PEELBOX_DETECTION_MODE=static
├── test_node_npm_llm()
├── test_node_npm_static()
└── ... (50+ tests total for 25+ fixtures)
```

**All tests remain e2e tests** - they spawn the peelbox binary and validate JSON output.

## Architecture Changes

### 1. Detection Mode Environment Variable

```rust
// src/config.rs

/// Detection mode controls LLM usage in pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionMode {
    /// Full detection: LLM + static analysis (default)
    Full,
    /// Static analysis only, no LLM calls
    StaticOnly,
    /// LLM-only detection (for testing LLM path specifically)
    LLMOnly,
}

impl DetectionMode {
    pub fn from_env() -> Self {
        match env::var("PEELBOX_DETECTION_MODE")
            .unwrap_or_else(|_| "full".to_string())
            .to_lowercase()
            .as_str()
        {
            "static" => DetectionMode::StaticOnly,
            "llm" => DetectionMode::LLMOnly,
            "full" | _ => DetectionMode::Full,
        }
    }
}
```

### 2. CLI Integration

```rust
// src/main.rs (or src/cli/detect.rs)

pub async fn run_detect(args: &DetectArgs) -> Result<()> {
    // Read detection mode from environment
    let mode = DetectionMode::from_env();

    // Create LLM client (if needed)
    let llm_client = if mode == DetectionMode::StaticOnly {
        // No LLM needed for static-only mode
        Arc::new(NoOpLLMClient::new())
    } else {
        // Create real LLM client
        select_llm_client().await?
    };

    // Create detection service with mode
    let service = DetectionService::new(llm_client)?;

    // Execute pipeline with mode
    let results = service.detect_with_mode(&args.path, mode).await?;

    // Output results
    output_results(&results, &args.format)?;

    Ok(())
}
```

### 3. Pipeline Orchestrator with Mode

```rust
// src/pipeline/orchestrator.rs

impl PipelineOrchestrator {
    pub async fn execute_with_mode(
        &self,
        repo_path: &Path,
        mode: DetectionMode,
    ) -> Result<Vec<UniversalBuild>> {
        // Phase 1: Scan (always runs, no LLM)
        let scan = scan::execute(repo_path)?;

        // Phase 2: Classify
        let classification = classify::execute(
            self.llm_client.as_ref(),
            &scan,
            &self.heuristic_logger,
            mode  // Pass mode to phase
        ).await?;

        // ... other phases also receive mode parameter
    }
}
```

### 4. Phase Execution with Mode

**Example: Classify Phase**

```rust
// src/pipeline/phases/02_classify.rs

pub async fn execute(
    llm_client: &dyn LLMClient,
    scan: &ScanResult,
    logger: &HeuristicLogger,
    mode: DetectionMode,
) -> Result<ClassifyResult> {
    // Always try deterministic first
    if can_classify_deterministically(scan) {
        return Ok(deterministic_classify(scan));
    }

    // If StaticOnly mode, return best-effort static result
    if mode == DetectionMode::StaticOnly {
        return Ok(deterministic_classify(scan)); // May be lower confidence
    }

    // Otherwise use LLM
    query_llm_with_logging(llm_client, prompt, 1000, "classify", logger).await
}
```

**All phases with LLM calls follow this pattern:**
1. Try deterministic/static path first
2. If `StaticOnly` mode, return deterministic result (even if low confidence)
3. Otherwise, call LLM

### 5. E2e Test Structure

```rust
// tests/e2e.rs

/// Helper to run detection with specific mode
fn run_detection_with_mode(
    fixture: PathBuf,
    test_name: &str,
    mode: &str,
) -> Result<Vec<UniversalBuild>, String> {
    // Create .git directory in fixture
    let git_dir = fixture.join(".git");
    if !git_dir.exists() {
        std::fs::create_dir_all(&git_dir).ok();
    }

    let output = Command::new(peelbox_bin())
        .env("PEELBOX_DETECTION_MODE", mode)  // Set detection mode
        .env("PEELBOX_PROVIDER", "embedded")   // Use embedded LLM if needed
        .env("PEELBOX_MODEL_SIZE", "7B")
        .arg("detect")
        .arg(fixture)
        .arg("--format")
        .arg("json")
        .output()
        .expect("Failed to execute peelbox");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON output
    if let Ok(results) = serde_json::from_str::<Vec<UniversalBuild>>(&stdout) {
        return Ok(results);
    }
    if let Ok(result) = serde_json::from_str::<UniversalBuild>(&stdout) {
        return Ok(vec![result]);
    }

    Err(format!("Failed to parse output as JSON: {}", stdout))
}

/// Helper for LLM mode
fn run_detection_llm(fixture: PathBuf, test_name: &str) -> Result<Vec<UniversalBuild>, String> {
    run_detection_with_mode(fixture, test_name, "llm")
}

/// Helper for static mode
fn run_detection_static(fixture: PathBuf, test_name: &str) -> Result<Vec<UniversalBuild>, String> {
    run_detection_with_mode(fixture, test_name, "static")
}

//
// Dual-mode tests for each fixture
//

#[test]
#[serial]
fn test_rust_cargo_llm() {
    let fixture = fixture_path("single-language", "rust-cargo");
    let results = run_detection_llm(fixture, "e2e_test_rust_cargo_llm")
        .expect("Detection failed");

    assert_detection(&results, "cargo", "rust-cargo");
}

#[test]
#[serial]
fn test_rust_cargo_static() {
    let fixture = fixture_path("single-language", "rust-cargo");
    let results = run_detection_static(fixture, "e2e_test_rust_cargo_static")
        .expect("Detection failed");

    assert_detection(&results, "cargo", "rust-cargo");
}

// ... repeat for all fixtures
```

### 6. NoOpLLMClient for Static Mode

```rust
// src/llm/noop.rs

/// LLM client that never makes actual calls, used for StaticOnly mode
pub struct NoOpLLMClient;

impl NoOpLLMClient {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LLMClient for NoOpLLMClient {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, BackendError> {
        // This should never be called in StaticOnly mode
        Err(BackendError::ConfigError(
            "LLM called in StaticOnly mode! All phases should use deterministic paths.".to_string()
        ))
    }

    fn name(&self) -> &str {
        "NoOpLLMClient"
    }
}
```

**Purpose**: If any phase incorrectly calls LLM in StaticOnly mode, the binary returns an error with clear message.

## Data Flow

### E2e Test - LLM Mode
```
1. Test spawns peelbox binary with PEELBOX_DETECTION_MODE=llm
2. Binary reads env var, sets DetectionMode::LLMOnly
3. Creates embedded LLM client
4. Pipeline executes, phases use LLM when needed
5. Binary outputs JSON to stdout
6. Test parses JSON and validates against expected output
```

### E2e Test - Static Mode
```
1. Test spawns peelbox binary with PEELBOX_DETECTION_MODE=static
2. Binary reads env var, sets DetectionMode::StaticOnly
3. Creates NoOpLLMClient (returns error if called)
4. Pipeline executes, phases skip LLM and use deterministic paths
5. Binary outputs JSON to stdout
6. Test parses JSON and validates against expected output (may have lower confidence)
```

### E2e Test - Full Mode (Default)
```
1. Test spawns peelbox binary (no PEELBOX_DETECTION_MODE set, defaults to "full")
2. Binary uses DetectionMode::Full
3. Creates embedded LLM client
4. Pipeline executes, tries static first, falls back to LLM as needed
5. Binary outputs JSON to stdout
6. Test parses JSON and validates
```

## Expected Output Strategy

**Mode-Specific Expected Files:**
- `rust-cargo.json` - Full mode expectations (existing)
- `rust-cargo-llm.json` - LLM mode expectations (optional, can reuse full mode)
- `rust-cargo-static.json` - Static mode expectations (may differ in confidence/completeness)

**Loading Strategy:**
```rust
fn load_expected(fixture_name: &str, mode: &str) -> Option<Vec<UniversalBuild>> {
    // Try mode-specific file first
    let mode_specific = format!("{}-{}.json", fixture_name, mode);
    if let Some(output) = try_load(mode_specific) {
        return Some(output);
    }

    // Fall back to generic expected output
    let generic = format!("{}.json", fixture_name);
    try_load(generic)
}
```

## Test Organization

```
tests/
├── e2e.rs                     # All e2e tests (50+ dual-mode tests)
│   ├── LLM mode tests (spawn CLI with PEELBOX_DETECTION_MODE=llm)
│   ├── Static mode tests (spawn CLI with PEELBOX_DETECTION_MODE=static)
│   └── Shared helpers (run_detection_llm, run_detection_static)
│
├── cli_integration.rs         # CLI integration tests (unchanged)
├── mock_detection_test.rs     # MockLLMClient tests (unchanged)
└── fixtures/
    ├── expected/
    │   ├── rust-cargo.json           # Full/LLM mode
    │   ├── rust-cargo-static.json    # Static mode (if different)
    │   └── ...
    └── single-language/
        └── ...
```

## Implementation Strategy

### Phase 1: CLI Mode Control
- Add `DetectionMode` enum
- Add environment variable parsing
- Pass mode from CLI to PipelineOrchestrator

### Phase 2: Pipeline Integration
- Update PipelineOrchestrator to accept mode
- Update each phase to respect mode parameter
- Add NoOpLLMClient for static mode

### Phase 3: E2e Test Helpers
- Add `run_detection_llm()` and `run_detection_static()` helpers
- Update validation to handle mode-specific expectations

### Phase 4: Add Dual Tests
- Create LLM and static variants for all 25+ fixtures
- 50+ e2e tests total

### Phase 5: Expected Outputs
- Create static-mode expected JSON where needed
- Document differences

### Phase 6: Documentation
- Document how to run tests in different modes
- Update CLAUDE.md with examples

## Benefits

1. **Fast CI**: Static mode e2e tests run without LLM backend (< 10 seconds for all tests)
2. **Complete Coverage**: Both LLM and static paths tested via CLI
3. **True E2e**: All tests validate the full binary, not just library code
4. **Simple**: All tests use same pattern (spawn binary with env var)
5. **Deterministic**: Static mode is fully deterministic
6. **Clear Failures**: NoOpLLMClient catches incorrect LLM usage

## Trade-offs

### More E2e Tests
- **Before**: 25 e2e tests
- **After**: 50+ e2e tests (25 fixtures × 2 modes)
- **Mitigation**: Shared helpers reduce duplication, tests run in parallel

### Slower Than Unit Tests
- E2e tests spawn binary, slower than unit tests
- **Mitigation**: Static mode is still fast (< 10 seconds total), no LLM overhead

## Success Metrics

- [ ] 50+ e2e tests (25 fixtures × 2 modes)
- [ ] All tests spawn CLI binary
- [ ] CLI respects `PEELBOX_DETECTION_MODE` environment variable
- [ ] Static mode tests run without LLM backend
- [ ] Static mode tests complete in < 10 seconds
- [ ] All tests pass in all modes
