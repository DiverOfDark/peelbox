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

## Phase 1: Embedded LLM Support (Phi-4-mini-reasoning)

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
3. Implement Phi-4-mini-reasoning model download:
   - Auto-download to `~/.aipack/models/`
   - GGUF format for efficient CPU inference
   - Model source: Hugging Face (microsoft/phi-4-mini-reasoning)
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

### Success Criteria
- Model downloads automatically on first run
- Detection completes in <10s on modern CPU
- Works with jumpstart data for optimal context

---

## Phase 2: universalbuild.yaml Output Format

**Priority**: HIGH
**Duration**: 4-6 days
**Owner**: TBD

### Objective
Transform detection results into the new declarative build format for container builds.

### Tasks
1. Create `src/output/universalbuild.rs` module
2. Define data structures:
   ```rust
   pub struct UniversalBuild {
       version: String,
       build: BuildStage,
       runtime: RuntimeStage,
   }

   pub struct BuildStage {
       base: String,
       packages: Vec<String>,
       env: HashMap<String, String>,
       command: Vec<String>,
       context: Vec<String>,
       cache: Vec<String>,
       artifacts: Vec<String>,
   }

   pub struct RuntimeStage {
       base: String,
       packages: Vec<String>,
       env: HashMap<String, String>,
       copy: Vec<CopySpec>,
       command: Vec<String>,
       ports: Vec<u16>,
       healthcheck: Option<Healthcheck>,
   }
   ```
3. Implement conversion from `DetectionResult` to `UniversalBuild`
4. Add YAML serialization with schema validation
5. Support multi-file output for monorepos (jumpstart detects structure)
6. Update CLI to generate universalbuild.yaml files
7. Add examples for common languages/frameworks:
   - Node.js (npm, yarn, pnpm)
   - Rust (Cargo)
   - Go (go mod)
   - Java (Maven, Gradle)
   - Python (pip, poetry)

### Expected Outcomes
- Declarative build format for all detected projects
- Direct integration with Appbahn platform
- Container build without Dockerfile

### Success Criteria
- Valid YAML output for 100% of detections
- Schema matches PRD-2.0 specification
- Successfully builds containers using generated specs

---

## Phase 3: External Memory System

**Priority**: HIGH
**Duration**: 3-5 days
**Owner**: TBD

### Objective
Implement persistent state tracking for large repository exploration.

### Tasks
1. Create `src/detection/memory.rs` module
2. Define data structures:
   ```rust
   pub struct ExternalMemory {
       exploration_log: Vec<ExplorationStep>,
       discovered_projects: Vec<ProjectInfo>,
       workspace_info: Option<WorkspaceInfo>,
       next_steps: Vec<String>,
   }

   pub struct ExplorationStep {
       timestamp: u64,
       action: String,
       result: String,
       confidence: f32,
   }

   pub struct ProjectInfo {
       path: String,
       language: String,
       build_system: String,
       confidence: f32,
   }
   ```
3. Add memory initialization from jumpstart data
4. Integrate memory query/update tools for LLM:
   - `query_memory` - Read current state
   - `update_memory` - Add discoveries
5. Add memory persistence between tool calls (in-memory during detection session)
6. Update system prompts to use memory effectively
7. Add memory serialization/deserialization (JSON format)

### Expected Outcomes
- Handle monorepos with 50+ projects without context overflow
- LLM can track exploration state across iterations
- Reduced redundant work

### Success Criteria
- Memory persists across all tool calls in a session
- Jumpstart data seeds initial memory state
- Memory queries count as tool calls (tracked in limits)

---

## Phase 4: Hierarchical Exploration Strategy

**Priority**: MEDIUM
**Duration**: 3-4 days
**Owner**: TBD

### Objective
Optimize large repository analysis with progressive depth levels.

### Tasks
1. Create `src/detection/strategy.rs` module
2. Implement exploration levels:
   - **Level 0: Structural Survey** (use jumpstart data as foundation)
   - **Level 1: Manifest Discovery** (augment jumpstart with focused search if needed)
   - **Level 2: Manifest Inspection** (selective reading, 2KB truncation)
   - **Level 3: Context Refinement** (deep dive for low confidence <80%)
3. Add confidence-based early termination logic
4. Update LLM prompts with level-aware instructions
5. Skip levels when jumpstart provides high confidence:
   - Single `package.json` → skip to Level 2
   - Clear monorepo structure → skip to Level 1
6. Add metrics tracking per level

### Expected Outcomes
- Jumpstart eliminates Level 0-1 for most repos
- Only complex/ambiguous repos need Level 3
- Average exploration depth: Level 2

### Success Criteria
- 80% of repos complete at Level 1-2
- Level 3 only triggered for <20% of detections
- No accuracy loss vs exhaustive exploration

---

## Phase 5: Monorepo Multi-Agent System

**Priority**: MEDIUM
**Duration**: 4-5 days
**Owner**: TBD

### Objective
Enable parallel analysis of monorepo sub-projects.

### Tasks
1. Enhance `src/detection/service.rs` with agent orchestration
2. Implement root agent for monorepo detection:
   - Use jumpstart workspace detection
   - Identify sub-project boundaries
3. Implement sub-agent spawning per project:
   - Based on jumpstart discovered projects
   - Each agent gets isolated context
4. Add parallel execution with concurrency limits:
   - Use `tokio::spawn`
   - Limit to 4-8 concurrent agents
5. Implement result aggregation
6. Generate multiple universalbuild.yaml files (one per project)
7. Add progress reporting for monorepo analysis

### Expected Outcomes
- Parallel analysis of monorepo sub-projects
- Linear scaling with project count (up to concurrency limit)
- Complete discovery of all buildable projects

### Success Criteria
- 100% discovery rate for monorepo projects
- Latency <30s for monorepos with <10 projects
- Latency scales linearly beyond 10 projects

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
   - Phi-4-mini-reasoning: 16K tokens
   - Qwen 2.5 Coder 7B: 32K tokens
   - Claude Sonnet: 200K tokens
   - GPT-4: 128K tokens
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
- Embedded LLM (16K context) handles repositories with >50 files

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
