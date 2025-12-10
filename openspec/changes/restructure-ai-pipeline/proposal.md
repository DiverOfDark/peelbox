# Change: Restructure AI Analysis Pipeline

## Why

The current AI analysis implementation is a monolithic 350+ line method in `GenAIBackend.detect()` that handles conversation management, tool execution, validation, caching, and error handling all in one place. This makes the code:

- **Hard to test**: Cannot unit test individual components without mocking the entire LLM
- **Hard to maintain**: Changes in one area risk breaking others
- **Hard to extend**: Adding new tools or providers requires touching multiple files
- **Hard to debug**: No visibility into what's happening during analysis

## What Changes

Complete restructure of the AI analysis pipeline into layered components:

1. **New `pipeline/` module** - `AnalysisPipeline` orchestrator with progress events
2. **New `conversation/` module** - `ConversationManager` for LLM message handling
3. **New `llm/` module** - `LLMClient` trait abstracting provider communication
4. **New `fs/` module** - `FileSystem` trait for testable file operations
5. **New `validation/` module** - Centralized validation logic
6. **Restructured `tools/` module** - Unified `ToolSystem` with co-located definitions

**Key architectural changes:**
- Trait-based abstractions (`LLMClient`, `FileSystem`) enable unit testing
- Event-based progress reporting for observability
- Each tool defined in its own file with schema + execution together
- Clear separation between orchestration, communication, and execution

## Impact

- **Affected specs:** ai-pipeline (new capability)
- **Affected code:**
  - `src/ai/` → replaced by `src/llm/`
  - `src/detection/tools/` → replaced by `src/tools/`
  - `src/detection/service.rs` → simplified, delegates to pipeline
  - `src/detection/prompt.rs` → moved to `src/conversation/`
  - `src/detection/jumpstart/` → moved to `src/jumpstart/`
- **External API:** Unchanged - `DetectionService` interface preserved
- **Breaking internal changes:** All internal APIs restructured
