# Project Context

## Purpose

aipack is an AI-powered buildkit frontend for intelligent build command detection. It analyzes repositories on-demand using LLM function calling with iterative tool execution to detect build systems and generate multi-stage container build specifications (UniversalBuild). The tool avoids context window limitations by having the LLM explore repositories incrementally rather than passing full repository context upfront.

## Tech Stack

- **Language**: Rust 1.70+
- **Build System**: Cargo
- **Async Runtime**: tokio
- **AI/LLM**: GenAI crate (unified multi-provider client)
  - Supported providers: Ollama (local), Anthropic Claude, OpenAI GPT, Google Gemini, xAI Grok, Groq
- **HTTP Client**: reqwest (async)
- **CLI Framework**: clap (derive macros)
- **Error Handling**: anyhow, thiserror
- **Serialization**: serde, serde_json
- **Logging**: tracing

## Project Conventions

### Code Style

- Run `cargo fmt` before committing
- Address all `cargo clippy` warnings
- Use meaningful variable names
- Keep functions under 50 lines when possible
- **No backwards compatibility**: Breaking changes are acceptable when they improve the codebase
- **No historical comments**: Code should reflect current state only, no comments explaining what was added/removed/changed
- **Minimal comments**: Only comment non-obvious logic; this is not a library requiring examples

### Architecture Patterns

- **Tool-based detection**: LLM explores repositories iteratively using 6 specialized tools (`list_files`, `read_file`, `search_files`, `get_file_tree`, `grep_content`, `submit_detection`)
- **Backend trait**: All LLM integrations implement `LLMBackend` trait for pluggable backends
- **Async-first**: All I/O operations are async using tokio
- **Error propagation**: Use `anyhow::Result<T>` for application errors, `thiserror::Error` for custom error types

### Testing Strategy

- **Unit tests**: Test individual functions in `#[cfg(test)]` modules near the code
- **Integration tests**: Full workflow tests in `tests/` directory
- **Test fixtures**: Use `tests/fixtures/` for test repositories
- Run all tests with `cargo test`
- Coverage with `cargo tarpaulin --out Html`

### Git Workflow

- Commit message format: `feat:`, `fix:`, `docs:`, `chore:`, `test:`, `perf:`
- Run `cargo fmt && cargo clippy && cargo test` before commits
- Update CHANGELOG.md with changes

## Domain Context

- **UniversalBuild**: Multi-stage container build specification with metadata, build stage, and runtime stage
- **Build detection**: Identifying project type (Rust/Cargo, Node.js/npm, Python/pip, etc.) and generating appropriate Dockerfile-like build instructions
- **Context window efficiency**: Designed to handle large repositories without exceeding LLM context limits

## Important Constraints

- LLM responses must be parsed as structured JSON
- Tool execution has configurable timeouts and iteration limits to prevent runaway loops
- File size limits prevent reading excessively large files
- Stop sequences prevent LLM self-reasoning loops

## External Dependencies

- **Ollama**: Local LLM inference server (default: localhost:11434)
- **Cloud LLM APIs**: Anthropic, OpenAI, Google, xAI, Groq (requires API keys via environment variables)
- **genai crate**: Handles multi-provider LLM communication and authentication
