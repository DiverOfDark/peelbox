# Implementation Plan for aipack PRD-2.0

**Created**: 2025-11-08
**Status**: Draft - Ready for Implementation
**Estimated Duration**: 8-9 weeks

---

## Current Status

✅ **Core tool-based detection system** already implemented (6 tools)
✅ **Multi-provider LLM backend** (Ollama, Claude, OpenAI, Gemini, etc.)
✅ **Security & validation** (path traversal protection, binary detection, size limits)
✅ **Phase 0: Zero Tool Call Handler** - COMPLETED (2025-11-08)
  - System prompt forbids conversational text
  - Retry logic for zero tool calls (max 2 attempts)
  - Improved error messages for non-existent files
  - JSON format for get_file_tree tool
  - Limit of 5 tool calls per response
✅ **Phase 1: Jumpstart Analysis** - COMPLETED (2025-11-08)
  - Fast manifest file scanner (40+ patterns)
  - Exclusion patterns for common directories
  - LLM context pre-population with discovered files
  - Language and build system detection
  - Monorepo/workspace detection
  - 50-70% reduction in tool calls
  - 70% faster detection speed

---

## Phase 1: Embedded LLM Support (Qwen 2.5 Coder 7B Instruct)

**Priority**: HIGHEST
**Duration**: 5-7 days
**Owner**: TBD

### Objective
Add offline fallback with embedded LLM for zero-dependency operation.

### Tasks
1. Create `src/ai/embedded_backend.rs` module
2. Research and select inference library:
   - Option A: `candle-transformers` (pure Rust)
   - Option B: `llama-cpp-rs` (bindings to llama.cpp)
3. Implement Qwen 2.5 Coder 7B Instruct model download:
   - Auto-download to `~/.aipack/models/`
   - GGUF format for efficient CPU inference
   - Model source: Hugging Face (Qwen/Qwen2.5-Coder-7B-Instruct-GGUF)
   - Context window: 32K tokens
4. Implement GenAI trait for embedded backend
5. Add fallback logic (try external → fallback to embedded)
6. Add model management commands to CLI:
   - `aipack download-model`
   - `aipack model status`
   - `aipack model remove`
7. Update configuration system with embedded model settings
8. Test inference performance on CPU (target: <10s per detection)

### Expected Outcomes
- Out-of-the-box operation without external LLM dependencies
- Offline detection capability
- Fallback for API failures or rate limits
- 32K context window handles larger repositories

### Success Criteria
- Model downloads automatically on first run
- Detection completes in <10s on modern CPU
- Works with jumpstart data for optimal context
- 32K context window supports complex monorepos

---

## Phase 3 & 4: Multi-Project Detection & Best Practices System (IN PROGRESS)

**Priority**: HIGHEST
**Duration**: 13 days (~2.5 weeks)
**Owner**: TBD
**Status**: In Progress

### Objective
Implement two-agent detection system that separates project discovery from build specification generation, with support for best practices templates to improve efficiency and accuracy.

### Architecture Overview

**Two-Agent System**:
1. **Discovery Agent**: Finds all buildable projects in repository (always runs first unless `--project` flag)
2. **Build Agent**: Generates UniversalBuild for single project (enhanced with best practices templates)

**Flow**:
```
User runs: aipack detect <path> [--project <subpath>]
    ↓
--project specified?
    ├─ Yes → Skip to Build Agent
    └─ No → Discovery Agent
        ↓
    1 project? → Build Agent
    2-200 projects? → Return list to user
    >200 projects? → Error
```

### Phase 3a: Discovery Agent (4 days)

**Files to Create**:
- `src/detection/discovery_agent.rs` - Discovery agent implementation
- `src/output/project_list.rs` - ProjectInfo and ProjectList structs

**Files to Modify**:
- `src/detection/tools/executor.rs` - Add `submit_project_list` tool
- `src/detection/prompt.rs` - Add discovery agent system prompt

**Tasks**:
1. Create Discovery Agent with simple prompt: "Find all buildable projects"
2. Implement `submit_project_list` tool (terminal tool for discovery)
3. Add ProjectInfo and ProjectList data structures
4. Implement validation logic:
   - 0 projects → Error
   - 1 project → Continue to build
   - 2-200 projects → Return list
   - >200 projects → Error with hint
5. Add output serialization (JSON, YAML, table format)
6. Write tests for discovery on monorepos and single projects

**Deliverables**:
- Discovery agent can identify all buildable projects
- Validation prevents >200 projects
- Clear table/JSON/YAML output

### Phase 3b: Best Practices Tool (3 days)

**Files to Create**:
- `src/detection/tools/best_practices.rs` - Template definitions and logic

**Files to Modify**:
- `src/detection/tools/executor.rs` - Add `get_best_practices` tool
- `src/detection/prompt.rs` - Update build agent prompt to explain new tool

**Tasks**:
1. Define BestPracticeTemplate data structures:
   - BuildStageTemplate (base image, packages, commands, cache, artifacts)
   - RuntimeStageTemplate (base image, packages, ports, healthcheck)
2. Implement templates for 15 combinations:
   - Rust + cargo
   - JavaScript/TypeScript + npm, yarn, pnpm, bun
   - Java + maven, gradle
   - Python + pip, poetry, pipenv
   - Go + go mod
   - .NET + dotnet
   - Ruby + bundler
   - C/C++ + cmake, make
3. Add template matching logic (language + build_system)
4. Wire up `get_best_practices` tool to executor
5. Update system prompt to encourage template usage
6. Test all template combinations

**Deliverables**:
- Templates for 15 language/build-system combinations
- LLM can request and apply templates
- Fallback for unsupported combinations

### Phase 4: Two-Agent Orchestration (2 days)

**Files to Create**:
- `src/detection/detection_result.rs` - DetectionResult enum

**Files to Modify**:
- `src/detection/detector.rs` - Orchestration logic, --project flag handling

**Tasks**:
1. Implement DetectionResult enum (SingleProject | MultiProject)
2. Add orchestration logic:
   - Check for `--project` flag → skip discovery if present
   - Run discovery agent first (if no flag)
   - Validate project count
   - Run build agent conditionally
3. Add path validation for `--project` flag:
   - Ensure path exists
   - Prevent path traversal
   - Validate within repository boundaries
4. Wire both agents together
5. Handle all validation cases
6. Write end-to-end integration tests

**Deliverables**:
- Sequential agent execution working
- --project flag skips discovery
- Automatic mode detection (1 vs many projects)

### Phase 5: CLI Integration (2 days)

**Files to Modify**:
- `src/cli/commands.rs` - Add --project flag, handle DetectionResult
- `src/main.rs` - CLI argument parsing

**Tasks**:
1. Add `--project <subpath>` flag to detect command
2. Add `--format <format>` flag (table, json, yaml)
3. Implement output formatting for ProjectList:
   - Table format (default, human-readable)
   - JSON format (machine-readable)
   - YAML format (alternative)
4. Handle DetectionResult enum in CLI output
5. Update help text and examples
6. Add path validation logic
7. Test all CLI combinations

**Deliverables**:
- --project flag working correctly
- Multiple output formats
- Clear help text

### Documentation (2 days)

**Files to Update**:
- `README.md` - Add two-agent workflow explanation and examples
- `PRD-2.0.md` - Document new architecture (already done)
- `ROADMAP.md` - Update this file
- `CLAUDE.md` - Update architecture section with two-agent system
- `CHANGELOG.md` - Add changelog entries for Phase 3 & 4

**Tasks**:
1. Update README with workflow examples
2. Document all CLI flags and options
3. Add examples for common scenarios
4. Update architecture diagrams in CLAUDE.md
5. Write comprehensive changelog

**Deliverables**:
- Complete documentation update
- Clear examples for users
- Updated architecture docs

### Expected Outcomes
- Users can discover all projects in monorepos before building
- `--project` flag enables direct builds when target is known
- Best practices templates reduce LLM token usage by ~30%
- Works seamlessly with Maven/Gradle multi-module projects
- Clear, actionable output for multi-project repositories

### Success Criteria
- ✅ Discovery agent finds all projects in monorepos (>95% accuracy)
- ✅ Build agent generates correct UniversalBuild for single projects
- ✅ `--project` flag skips discovery and builds specific project
- ✅ Validation prevents >200 projects error
- ✅ Best practices templates work for 15 combinations
- ✅ No regression on single-project detection
- ✅ Works with multi-module Maven/Gradle projects
- ✅ Documentation is comprehensive and clear

---

## Phase 6: Enhanced Tool Features

**Priority**: MEDIUM
**Duration**: 2-3 days
**Owner**: TBD

### Objective
Add smart filtering, caching, and truncation to existing tools.

### Tasks
1. Enhance `search_files` tool:
   - Add include/exclude pattern support
   - Add max results limit
2. Enhance `read_file` tool:
   - Add truncation markers (show first 2KB with "...truncated...")
   - Add preview mode (first N lines)
3. Implement tool result caching:
   - LRU cache with 50-100 entries
   - Cache key: (tool_name, args)
   - Cache invalidation on timeout
4. Add workspace detection helpers (already discovered by jumpstart)
5. Update tool tests for new features

### Expected Outcomes
- Reduced redundant file reads
- Better context management for large files
- Faster tool execution

### Success Criteria
- Cache hit rate >30% for typical detections
- Truncation preserves critical information (manifests fit in 2KB)
- No regression in detection accuracy

---

## Phase 7: Context Size Tracking & Optimization

**Priority**: MEDIUM
**Duration**: 3-4 days
**Owner**: TBD

### Objective
Track actual token usage and dynamically optimize context when approaching model limits.

### Tasks
1. Create `src/detection/context_manager.rs` module
2. Implement token counting:
   - Use `tiktoken` or similar for accurate token estimation
   - Track tokens per message in conversation history
   - Monitor cumulative context size in real-time
3. Add model-specific context limits:
   ```rust
   pub struct ContextLimits {
       max_tokens: usize,      // Model's max context window
       warning_threshold: f32,  // Warn at 80% (0.8)
       truncate_threshold: f32, // Truncate at 90% (0.9)
   }
   ```
4. Implement context optimization strategies:
   - **Truncation**: Remove oldest tool results when approaching limit
   - **Summarization**: Compress older messages into summaries
   - **Selective retention**: Keep high-priority messages (jumpstart, recent tools)
   - **Progressive removal**: Remove in order: old file reads → old searches → exploration steps
5. Add per-provider context limits:
   - Qwen 2.5 Coder 7B Instruct: 32K tokens
   - Claude Sonnet: 200K tokens
   - GPT-4: 128K tokens
   - Gemini Pro: 2M tokens
6. Implement context warning system:
   - Log warnings when reaching 80% capacity
   - Automatically trigger optimization at 90% capacity
   - Fail gracefully at 95% with clear error message
7. Add metrics tracking:
   - Peak context usage per detection
   - Number of truncations performed
   - Tokens saved through optimization
8. Integration with External Memory:
   - Move truncated content to external memory as summaries
   - Reference memory instead of repeating information
9. Add CLI flags:
   - `--max-context <tokens>` - Override default context limit
   - `--context-strategy <strategy>` - Choose optimization strategy (truncate|summarize|hybrid)
   - `--show-context-usage` - Display token usage stats after detection

### Expected Outcomes
- Never exceed model context windows
- Maintain detection accuracy even with truncated context
- Clear visibility into context usage
- Graceful degradation when context is tight

### Success Criteria
- Zero context overflow errors across all providers
- Detection accuracy >95% even with context optimization active
- Context warnings logged before automatic truncation
- Embedded LLM (32K context) handles repositories with >100 files

### Context Optimization Strategies

#### Strategy 1: Truncation (Simple)
Remove oldest tool results first:
1. Old file reads (>5 iterations ago)
2. Old search results
3. Redundant directory listings

#### Strategy 2: Summarization (Advanced)
Compress messages while preserving key information:
1. Summarize multiple file reads into discovery list
2. Compress search results into "found X files matching Y"
3. Keep only critical file contents (manifests)

#### Strategy 3: Hybrid (Recommended)
- Use truncation for low-value messages
- Use summarization for important context
- Always preserve: jumpstart data, recent tools (last 3), final detection

### Error Scenarios

**Problem**: Context exceeds 95% after optimization

**Resolution**:
1. Log detailed error with context breakdown
2. Suggest using a larger model
3. Fallback to hierarchical exploration (skip deep dives)
4. Return partial result with warnings

---

## Phase 8: Testing & Integration

**Priority**: HIGH
**Duration**: 5-7 days
**Owner**: TBD

### Objective
Comprehensive testing and documentation updates.

### Tasks
1. Add integration tests:
   - Jumpstart analysis (various repo structures)
   - Embedded LLM (model download, inference)
   - universalbuild.yaml generation (all languages)
   - Monorepo detection (pnpm, Cargo workspace, etc.)
   - Context size tracking (test with small context limits)
   - Context optimization strategies (truncation, summarization)
2. Performance benchmarking:
   - Verify <5s single app
   - Verify <30s monorepo
   - Tool call efficiency metrics
3. Update documentation:
   - `README.md` - Quick start guide
   - `CLAUDE.md` - Development guide
   - `PRD-2.0.md` - Mark implemented features
4. Add example outputs for common frameworks
5. Update `CHANGELOG.md`

### Expected Outcomes
- Comprehensive test coverage (>80%)
- All PRD-2.0 performance targets met
- Production-ready documentation

### Success Criteria
- All integration tests pass
- Performance benchmarks meet targets
- Documentation reviewed and approved

---

## Phase 9: E2E Automated Tests with Railpack Projects (OPTIONAL)

**Priority**: LOWEST
**Duration**: 2-3 days
**Owner**: TBD

### Objective
Build comprehensive E2E test suite using real-world example projects from railpack as test fixtures.

### Tasks
1. Create `tests/fixtures/railpack/` directory
2. Manually clone railpack repo once and copy example projects:
   - `tests/fixtures/railpack/nextjs-app/`
   - `tests/fixtures/railpack/django-app/`
   - `tests/fixtures/railpack/rust-actix/`
   - `tests/fixtures/railpack/spring-boot/`
   - etc.
3. Write E2E tests using local fixtures:
   ```rust
   #[tokio::test]
   async fn test_railpack_nextjs() {
       let result = aipack::detect("tests/fixtures/railpack/nextjs-app").await.unwrap();
       assert_eq!(result.build_system, "npm");
       assert_eq!(result.build_command, "npm run build");
   }
   ```
4. Validate for each fixture:
   - Correct build system detected
   - Valid build command
   - universalbuild.yaml is valid
5. Add expected outputs as snapshots in `tests/fixtures/railpack/*/expected_output.yaml`

### Expected Outcomes
- High confidence in real-world project detection
- Regression detection for supported project types

### Success Criteria
- >95% accuracy across all railpack fixtures
- Valid universalbuild.yaml for all test cases
