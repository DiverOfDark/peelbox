# Change: Migrate to Prompt Pipeline Architecture

## Why

The current tool-based agentic workflow has fundamental scalability and predictability limitations:

**Current Problems:**
1. **Context window bloat** - Tool schemas (7 tools) + conversation history + file contents quickly exceed limits
2. **Unpredictable token usage** - LLM decides exploration depth, can spiral into thousands of tokens
3. **Cannot target small models** - Agentic loop requires reasoning capability, preventing optimal use of embedded models (Qwen 2.5 Coder 0.5B/1.5B)
4. **Difficult to debug** - Failures happen mid-loop, unclear which decision went wrong
5. **No parallelization** - Sequential tool calls, slow for large repositories
6. **No optimization path** - Cannot extract heuristics to skip LLM calls for common patterns
7. **Monorepo handling** - Returns `Vec<UniversalBuild>` but analyzes sequentially, no per-service parallelization

## What Changes

Replace the tool-based agentic loop with a **multi-phase pipeline** where:

- **Code orchestrates the workflow**, not the LLM
- **Each LLM call is single-purpose** with minimal context (~150-500 tokens per prompt)
- **Sequential execution** - simple, linear processing (parallelization can be added later)
- **All prompts fit in 8k context window**, enabling use of smallest models
- **Deterministic parsers** handle known manifest formats (package.json, Cargo.toml, etc.) without LLM
- **LLM only used for unknowns**, with clear confidence scoring
- **Logging infrastructure** enables future heuristic extraction to skip LLM entirely

**Architecture Shift:**

```
BEFORE (Agentic Loop):
LLM → tool_call(list_files) → LLM → tool_call(read_file) → LLM → tool_call(read_file) → ... → submit_detection
(1 LLM conversation, 5-15 iterations, 10k-50k tokens, sequential)

AFTER (Pipeline):
Scan → Classify(Prompt0) → Structure(Prompt1) → Dependencies(Prompt2) → BuildOrder → ServiceAnalysis(Prompts 3-9) → Cache(deterministic) → Assemble
(9 distinct prompts, <500 tokens each, sequential execution, deterministic for known formats)
```

## Impact

**Affected specs:**
- `ai-pipeline` - Completely redefines how AI detection works
- New capability: `prompt-pipeline` - Phase-based orchestration

**Affected code:**
- `src/pipeline/` - Complete rewrite of `analysis.rs`, add new phase modules (each phase contains prompt + execution)
- `src/llm/` - Keep `LLMClient` trait unchanged
- `src/detection/` - Keep `DetectionService` (public API unchanged), add phase implementations
- `src/tools/` - Deprecated; logic moved to code-driven extraction (keep `get_best_practices` for templates)
- `src/languages/` - **Extend existing `LanguageDefinition` trait** to add dependency parsing methods
- `src/extractors/` - New module for code-based extraction (port, env vars, health)
- `src/heuristics/` - New module for future optimization
- `src/bootstrap/` - Leverage existing `BootstrapScanner` for initial language detection
- `src/fs/` - Use existing `FileSystem` trait for testability
- `src/validation/` - Keep existing validation system

**Migration strategy:**
- Incremental implementation - build pipeline phases one at a time
- Each phase tested independently before moving to next
- Replace tool-based approach incrementally, not big-bang
- Leverage existing tests with recording system for validation
- Remove deprecated tool code as phases are completed

**Benefits:**
- **85-95% token reduction** - From 10k-50k tokens to 1k-6k tokens per detection
- **Supports smallest models** - 8k context sufficient for all prompts
- **Predictable cost** - Fixed number of LLM calls (max 9, fewer with deterministic paths)
- **Debuggable** - Each phase has clear input/output, can inspect intermediate results
- **More deterministic** - Cache detection uses build system knowledge, not LLM guessing
- **Linear execution** - Simple sequential processing, easy to understand and debug
- **Future parallelization** - Can add parallelization later once pipeline is proven

**Risks:**
- Incremental refactor over 4-6 weeks
- Tests may need updates to match new phase-based architecture
- Some prompts may need tuning for accuracy

## Sequencing

This change depends on:
- `restructure-ai-pipeline` - Must complete first to have clean baseline

This change blocks:
- Future optimizations (heuristic extraction, caching)
- Support for smallest models (<1.5B parameters)
